use flame_macro::flame;
use font8x8::{UnicodeFonts, BASIC_FONTS};
use image::GenericImageView;
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

pub static TEXTURE_CACHE: Lazy<RwLock<HashMap<String, Arc<Texture>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

pub struct Texture {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
}

impl Texture {
    pub fn load(path: &str) -> Arc<Texture> {
        {
            let cache = TEXTURE_CACHE.read().unwrap();
            if let Some(tex) = cache.get(path) {
                return tex.clone();
            }
        }

        let tex = match image::open(path) {
            Ok(img) => {
                let width = img.width() as usize;
                let height = img.height() as usize;
                let mut pixels = Vec::with_capacity(width * height);
                for (_, _, pixel) in img.pixels() {
                    let r = pixel[0] as u32;
                    let g = pixel[1] as u32;
                    let b = pixel[2] as u32;
                    let a = pixel[3] as u32;
                    if a < 128 {
                        pixels.push(0);
                    } else {
                        pixels.push(0xFF000000 | (r << 16) | (g << 8) | b);
                    }
                }
                Arc::new(Texture {
                    width,
                    height,
                    pixels,
                })
            }
            Err(e) => {
                println!(
                    "Warning: Failed to load sprite {}: {}. Using fallback.",
                    path, e
                );
                let width = 50;
                let height = 50;
                let pixels = vec![0xFF0000; width * height]; // Red box fallback
                Arc::new(Texture {
                    width,
                    height,
                    pixels,
                })
            }
        };

        let mut cache = TEXTURE_CACHE.write().unwrap();
        cache.insert(path.to_string(), tex.clone());
        tex
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

impl Vector2 {
    pub fn new(x: f32, y: f32) -> Vector2 {
        Vector2 { x, y }
    }
    pub fn x(&self) -> f32 {
        self.x
    }
    pub fn y(&self) -> f32 {
        self.y
    }

    #[flame(rename = "setX")]
    pub fn set_x(&mut self, x: f32) {
        self.x = x;
    }

    #[flame(rename = "setY")]
    pub fn set_y(&mut self, y: f32) {
        self.y = y;
    }

    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn normalized(&self) -> Vector2 {
        let length = self.length();
        if length == 0.0 {
            return Vector2::new(0.0, 0.0);
        }
        Vector2::new(self.x / length, self.y / length)
    }
    pub fn distance(&self, other: &Vector2) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
    pub fn dot(&self, other: &Vector2) -> f32 {
        self.x * other.x + self.y * other.y
    }
}

#[derive(Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    pub fn x(&self) -> f32 { self.x }
    pub fn y(&self) -> f32 { self.y }
    pub fn width(&self) -> f32 { self.width }
    pub fn height(&self) -> f32 { self.height }
}

pub fn intersects(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}

#[derive(Clone)]
pub struct Sprite {
    pub position: Vector2,
    pub rotation: f32,
    pub scale: Vector2,
    pub visible: bool,
    texture: Arc<Texture>,
    transform_dirty: bool,
    scaled_pixels: Option<(usize, usize, Vec<u32>)>, // width, height, pixels
}

impl Sprite {
    pub fn new(path: String) -> Sprite {
        let texture = Texture::load(&path);
        Sprite {
            position: Vector2::new(0.0, 0.0),
            rotation: 0.0,
            scale: Vector2::new(1.0, 1.0),
            visible: true,
            texture,
            transform_dirty: false,
            scaled_pixels: None,
        }
    }

    #[flame(rename = "setPosition")]
    pub fn set_position(&mut self, position: &Vector2) {
        self.position = *position;
    }

    pub fn position(&self) -> Vector2 {
        self.position
    }

    #[flame(rename = "setRotation")]
    pub fn set_rotation(&mut self, rotation: f32) {
        self.rotation = rotation;
        self.transform_dirty = true;
    }

    #[flame(rename = "setScale")]
    pub fn set_scale(&mut self, scale: &Vector2) {
        self.scale = *scale;
        self.transform_dirty = true;
    }

    #[flame(rename = "setVisible")]
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub fn prepare(&mut self) {
        if !self.transform_dirty {
            return;
        }
        self.transform_dirty = false;

        if self.scale.x == 1.0 && self.scale.y == 1.0 && self.rotation == 0.0 {
            self.scaled_pixels = None;
            return;
        }

        let orig_w = self.texture.width as i32;
        let orig_h = self.texture.height as i32;
        let scale_x = self.scale.x.abs();
        let scale_y = self.scale.y.abs();

        let draw_w = (orig_w as f32 * scale_x) as i32;
        let draw_h = (orig_h as f32 * scale_y) as i32;

        if draw_w <= 0 || draw_h <= 0 {
            self.scaled_pixels = None;
            return;
        }

        let mut pixels = vec![0; (draw_w * draw_h) as usize];

        let step_x = (orig_w as f32 / draw_w as f32) * 65536.0;
        let step_y = (orig_h as f32 / draw_h as f32) * 65536.0;
        let step_x_int = step_x as u32;
        let step_y_int = step_y as u32;

        let mut src_y_fp = 0u32;
        for y in 0..draw_h {
            let src_y = (src_y_fp >> 16) as i32;
            let mut src_x_fp = 0u32;
            for x in 0..draw_w {
                let src_x = (src_x_fp >> 16) as i32;
                if src_x < orig_w && src_y < orig_h {
                    let color = self.texture.pixels[(src_y * orig_w + src_x) as usize];
                    pixels[(y * draw_w + x) as usize] = color;
                }
                src_x_fp += step_x_int;
            }
            src_y_fp += step_y_int;
        }

        self.scaled_pixels = Some((draw_w as usize, draw_h as usize, pixels));
    }
}

#[derive(Clone)]
pub struct Text {
    pub position: Vector2,
    pub content: String,
    pub color: u32,
    pub scale: i32,
    dirty: bool,
    cached_pixels: Vec<u32>,
    width: usize,
    height: usize,
}

impl Text {
    pub fn new(content: String) -> Text {
        let mut text = Text {
            position: Vector2::new(0.0, 0.0),
            content,
            color: 0xFFFFFF,
            scale: 3,
            dirty: true,
            cached_pixels: Vec::new(),
            width: 0,
            height: 0,
        };
        text.prepare();
        text
    }

    #[flame(rename = "setPosition")]
    pub fn set_position(&mut self, position: &Vector2) {
        self.position = *position;
    }

    #[flame(rename = "setContent")]
    pub fn set_content(&mut self, content: String) {
        if self.content != content {
            self.content = content;
            self.dirty = true;
        }
    }

    #[flame(rename = "setColor")]
    pub fn set_color(&mut self, r: i32, g: i32, b: i32) {
        let new_color = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        if self.color != new_color {
            self.color = new_color;
            self.dirty = true;
        }
    }

    #[flame(rename = "setScale")]
    pub fn set_scale(&mut self, scale: i32) {
        if self.scale != scale {
            self.scale = scale;
            self.dirty = true;
        }
    }

    #[flame(rename = "width")]
    pub fn get_width(&mut self) -> f32 {
        self.prepare();
        self.width as f32
    }
    
    #[flame(rename = "height")]
    pub fn get_height(&mut self) -> f32 {
        self.prepare();
        self.height as f32
    }

    pub fn prepare(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;

        let char_count = self.content.chars().count();
        let scale = self.scale as usize;
        self.width = char_count * 8 * scale;
        self.height = 8 * scale;

        self.cached_pixels.clear();
        self.cached_pixels.resize(self.width * self.height, 0);

        let mut curr_x = 0;
        for c in self.content.chars() {
            if let Some(glyph) = BASIC_FONTS.get(c) {
                for y in 0..8 {
                    for x in 0..8 {
                        if (glyph[y] & (1 << x)) != 0 {
                            for dy in 0..scale {
                                for dx in 0..scale {
                                    let px = curr_x + (x * scale) + dx;
                                    let py = (y * scale) + dy;
                                    self.cached_pixels[py * self.width + px] = 0xFF000000 | self.color;
                                }
                            }
                        }
                    }
                }
            }
            curr_x += 8 * scale;
        }
    }
}

pub struct Sound {
    path: String,
}

impl Sound {
    pub fn new(path: String) -> Result<Sound, String> {
        Ok(Sound { path })
    }
    pub fn play(&mut self) {
        println!("Playing sound: {}", self.path);
    }
    pub fn stop(&mut self) {
        println!("Stopping sound: {}", self.path);
    }
    #[flame(rename = "setVolume")]
    pub fn set_volume(&mut self, volume: f32) {
        println!("Set volume to: {}", volume);
    }
}

#[derive(Clone, Copy)]
pub struct Camera {
    pub position: Vector2,
    pub zoom: f32,
    pub width: f32,
    pub height: f32,
}

impl Camera {
    pub fn visible(&self, bounds: &Rect) -> bool {
        let cam_rect = Rect::new(self.position.x, self.position.y, self.width, self.height);
        intersects(&cam_rect, bounds)
    }
}

enum RenderCommand {
    Sprite(Sprite),
    Text(Text),
    RectFill(Rect, u32),
    RectFillRounded(Rect, u32, f32),
}

fn str_to_key(key_name: &str) -> Option<Key> {
    match key_name {
        "Key0" => Some(Key::Key0),
        "Key1" => Some(Key::Key1),
        "Key2" => Some(Key::Key2),
        "Key3" => Some(Key::Key3),
        "Key4" => Some(Key::Key4),
        "Key5" => Some(Key::Key5),
        "Key6" => Some(Key::Key6),
        "Key7" => Some(Key::Key7),
        "Key8" => Some(Key::Key8),
        "Key9" => Some(Key::Key9),
        "A" => Some(Key::A),
        "B" => Some(Key::B),
        "C" => Some(Key::C),
        "D" => Some(Key::D),
        "E" => Some(Key::E),
        "F" => Some(Key::F),
        "G" => Some(Key::G),
        "H" => Some(Key::H),
        "I" => Some(Key::I),
        "J" => Some(Key::J),
        "K" => Some(Key::K),
        "L" => Some(Key::L),
        "M" => Some(Key::M),
        "N" => Some(Key::N),
        "O" => Some(Key::O),
        "P" => Some(Key::P),
        "Q" => Some(Key::Q),
        "R" => Some(Key::R),
        "S" => Some(Key::S),
        "T" => Some(Key::T),
        "U" => Some(Key::U),
        "V" => Some(Key::V),
        "W" => Some(Key::W),
        "X" => Some(Key::X),
        "Y" => Some(Key::Y),
        "Z" => Some(Key::Z),
        "F1" => Some(Key::F1),
        "F2" => Some(Key::F2),
        "F3" => Some(Key::F3),
        "F4" => Some(Key::F4),
        "F5" => Some(Key::F5),
        "F6" => Some(Key::F6),
        "F7" => Some(Key::F7),
        "F8" => Some(Key::F8),
        "F9" => Some(Key::F9),
        "F10" => Some(Key::F10),
        "F11" => Some(Key::F11),
        "F12" => Some(Key::F12),
        "F13" => Some(Key::F13),
        "F14" => Some(Key::F14),
        "F15" => Some(Key::F15),
        "Down" => Some(Key::Down),
        "Left" => Some(Key::Left),
        "Right" => Some(Key::Right),
        "Up" => Some(Key::Up),
        "Apostrophe" => Some(Key::Apostrophe),
        "Backquote" => Some(Key::Backquote),
        "Backslash" => Some(Key::Backslash),
        "Comma" => Some(Key::Comma),
        "Equal" => Some(Key::Equal),
        "LeftBracket" => Some(Key::LeftBracket),
        "Minus" => Some(Key::Minus),
        "Period" => Some(Key::Period),
        "RightBracket" => Some(Key::RightBracket),
        "Semicolon" => Some(Key::Semicolon),
        "Slash" => Some(Key::Slash),
        "Backspace" => Some(Key::Backspace),
        "Delete" => Some(Key::Delete),
        "End" => Some(Key::End),
        "Enter" => Some(Key::Enter),
        "Escape" => Some(Key::Escape),
        "Home" => Some(Key::Home),
        "Insert" => Some(Key::Insert),
        "Menu" => Some(Key::Menu),
        "PageDown" => Some(Key::PageDown),
        "PageUp" => Some(Key::PageUp),
        "Pause" => Some(Key::Pause),
        "Space" => Some(Key::Space),
        "Tab" => Some(Key::Tab),
        "NumLock" => Some(Key::NumLock),
        "CapsLock" => Some(Key::CapsLock),
        "ScrollLock" => Some(Key::ScrollLock),
        "LeftShift" => Some(Key::LeftShift),
        "RightShift" => Some(Key::RightShift),
        "LeftCtrl" => Some(Key::LeftCtrl),
        "RightCtrl" => Some(Key::RightCtrl),
        "NumPad0" => Some(Key::NumPad0),
        "NumPad1" => Some(Key::NumPad1),
        "NumPad2" => Some(Key::NumPad2),
        "NumPad3" => Some(Key::NumPad3),
        "NumPad4" => Some(Key::NumPad4),
        "NumPad5" => Some(Key::NumPad5),
        "NumPad6" => Some(Key::NumPad6),
        "NumPad7" => Some(Key::NumPad7),
        "NumPad8" => Some(Key::NumPad8),
        "NumPad9" => Some(Key::NumPad9),
        "NumPadDot" => Some(Key::NumPadDot),
        "NumPadSlash" => Some(Key::NumPadSlash),
        "NumPadAsterisk" => Some(Key::NumPadAsterisk),
        "NumPadMinus" => Some(Key::NumPadMinus),
        "NumPadPlus" => Some(Key::NumPadPlus),
        "NumPadEnter" => Some(Key::NumPadEnter),
        "LeftAlt" => Some(Key::LeftAlt),
        "RightAlt" => Some(Key::RightAlt),
        "LeftSuper" => Some(Key::LeftSuper),
        "RightSuper" => Some(Key::RightSuper),
        "Unknown" => Some(Key::Unknown),
        _ => None,
    }
}

pub struct Game {
    title: String,
    width: u32,
    height: u32,
    window: Option<Window>,
    buffer: Vec<u32>,
    last_frame: Instant,
    dt: f32,
    render_queue: Vec<RenderCommand>,
    pub camera: Camera,
    target_fps: f64,
}

impl Game {
    pub fn new(title: String, width: u32, height: u32) -> Game {
        Game {
            title,
            width,
            height,
            window: None,
            buffer: vec![0; (width * height) as usize],
            last_frame: Instant::now(),
            dt: 0.0,
            render_queue: Vec::new(),
            camera: Camera {
                position: Vector2::new(0.0, 0.0),
                zoom: 1.0,
                width: width as f32,
                height: height as f32,
            },
            target_fps: 60.0,
        }
    }

    #[flame(rename = "setTargetFPS")]
    pub fn set_target_fps(&mut self, fps: f64) {
        self.target_fps = fps;
    }

    pub fn run(&mut self) {
        let window = Window::new(
            &self.title,
            self.width as usize,
            self.height as usize,
            WindowOptions::default(),
        )
        .unwrap();
        // We explicitly do NOT use limit_update_rate here, handling pacing ourselves.
        self.window = Some(window);
        self.last_frame = Instant::now();
    }

    pub fn running(&mut self) -> bool {
        if let Some(window) = &mut self.window {
            window.is_open() && !window.is_key_down(Key::Escape)
        } else {
            false
        }
    }

    #[flame(rename = "isKeyDown")]
    pub fn is_key_down(&self, key_name: String) -> bool {
        if let Some(window) = &self.window {
            if let Some(key) = str_to_key(&key_name) {
                window.is_key_down(key)
            } else {
                false
            }
        } else {
            false
        }
    }

    #[flame(rename = "isKeyPressed")]
    pub fn is_key_pressed(&self, key_name: String) -> bool {
        if let Some(window) = &self.window {
            if let Some(key) = str_to_key(&key_name) {
                window.is_key_pressed(key, KeyRepeat::No)
            } else {
                false
            }
        } else {
            false
        }
    }

    #[flame(rename = "isMouseDown")]
    pub fn is_mouse_down(&self) -> bool {
        if let Some(window) = &self.window {
            window.get_mouse_down(MouseButton::Left)
        } else {
            false
        }
    }

    #[flame(rename = "mouseX")]
    pub fn mouse_x(&self) -> f32 {
        if let Some(window) = &self.window {
            if let Some((x, _)) = window.get_mouse_pos(MouseMode::Discard) {
                return x + self.camera.position.x;
            }
        }
        0.0
    }

    #[flame(rename = "mouseY")]
    pub fn mouse_y(&self) -> f32 {
        if let Some(window) = &self.window {
            if let Some((_, y)) = window.get_mouse_pos(MouseMode::Discard) {
                return y + self.camera.position.y;
            }
        }
        0.0
    }

    #[flame(rename = "deltaTime")]
    pub fn delta_time(&self) -> f32 {
        self.dt
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        let mut dt = now.duration_since(self.last_frame).as_secs_f32();
        if dt > 0.1 {
            dt = 0.1;
        } // Cap
        self.dt = dt;
        self.last_frame = now;
    }

    pub fn clear(&mut self) {
        self.render_queue.clear();
        self.buffer.fill(0x222222);
    }

    pub fn draw(&mut self, sprite: &Sprite) {
        if sprite.visible {
            let mut s = sprite.clone();
            s.prepare();
            self.render_queue.push(RenderCommand::Sprite(s));
        }
    }

    #[flame(rename = "drawText")]
    pub fn draw_text(&mut self, text: &Text) {
        let mut t = text.clone();
        t.prepare();
        self.render_queue.push(RenderCommand::Text(t));
    }

    #[flame(rename = "drawRect")]
    pub fn draw_rect(&mut self, rect: &Rect, color: u32) {
        self.render_queue
            .push(RenderCommand::RectFill(*rect, color));
    }

    #[flame(rename = "drawRoundedRect")]
    pub fn draw_rounded_rect(&mut self, rect: &Rect, color: u32, radius: f32) {
        self.render_queue
            .push(RenderCommand::RectFillRounded(*rect, color, radius));
    }

    pub fn render(&mut self) {
        for cmd in &self.render_queue {
            match cmd {
                RenderCommand::Sprite(sprite) => {
                    let draw_w = if let Some((w, _, _)) = &sprite.scaled_pixels {
                        *w
                    } else {
                        sprite.texture.width
                    };
                    let draw_h = if let Some((_, h, _)) = &sprite.scaled_pixels {
                        *h
                    } else {
                        sprite.texture.height
                    };

                    let bounds = Rect::new(
                        sprite.position.x,
                        sprite.position.y,
                        draw_w as f32,
                        draw_h as f32,
                    );
                    if !self.camera.visible(&bounds) {
                        continue; // Frustum culling
                    }

                    let x_start = (sprite.position.x - self.camera.position.x) as i32;
                    let y_start = (sprite.position.y - self.camera.position.y) as i32;

                    if x_start >= self.width as i32
                        || y_start >= self.height as i32
                        || x_start + draw_w as i32 <= 0
                        || y_start + draw_h as i32 <= 0
                    {
                        continue;
                    }

                    // Fast blit
                    let src_pixels = if let Some((_, _, pixels)) = &sprite.scaled_pixels {
                        pixels.as_slice()
                    } else {
                        sprite.texture.pixels.as_slice()
                    };

                    for y in 0..draw_h {
                        let py = y_start + y as i32;
                        if py >= 0 && py < self.height as i32 {
                            for x in 0..draw_w {
                                let px = x_start + x as i32;
                                if px >= 0 && px < self.width as i32 {
                                    let color = src_pixels[y * draw_w + x];
                                    if (color & 0xFF000000) != 0 {
                                        self.buffer[(py * self.width as i32 + px) as usize] = color & 0x00FFFFFF;
                                    }
                                }
                            }
                        }
                    }
                }
                RenderCommand::Text(text) => {
                    let bounds = Rect::new(
                        text.position.x,
                        text.position.y,
                        text.width as f32,
                        text.height as f32,
                    );
                    if !self.camera.visible(&bounds) {
                        continue;
                    }

                    let x_start = (text.position.x - self.camera.position.x) as i32;
                    let y_start = (text.position.y - self.camera.position.y) as i32;

                    for y in 0..text.height {
                        let py = y_start + y as i32;
                        if py >= 0 && py < self.height as i32 {
                            for x in 0..text.width {
                                let px = x_start + x as i32;
                                if px >= 0 && px < self.width as i32 {
                                    let color = text.cached_pixels[y * text.width + x];
                                    if (color & 0xFF000000) != 0 {
                                        self.buffer[(py * self.width as i32 + px) as usize] = color & 0x00FFFFFF;
                                    }
                                }
                            }
                        }
                    }
                }
                RenderCommand::RectFill(rect, color) => {
                    if !self.camera.visible(rect) {
                        continue;
                    }
                    let x_start = (rect.x - self.camera.position.x) as i32;
                    let y_start = (rect.y - self.camera.position.y) as i32;
                    let w = rect.width as i32;
                    let h = rect.height as i32;

                    for y in 0..h {
                        let py = y_start + y;
                        if py >= 0 && py < self.height as i32 {
                            for x in 0..w {
                                let px = x_start + x;
                                if px >= 0 && px < self.width as i32 {
                                    self.buffer[(py * self.width as i32 + px) as usize] = *color;
                                }
                            }
                        }
                    }
                }
                RenderCommand::RectFillRounded(rect, color, radius) => {
                    if !self.camera.visible(rect) {
                        continue;
                    }
                    let x_start = (rect.x - self.camera.position.x) as i32;
                    let y_start = (rect.y - self.camera.position.y) as i32;
                    let w = rect.width as i32;
                    let h = rect.height as i32;
                    let rad = *radius as i32;

                    for y in 0..h {
                        let py = y_start + y;
                        if py >= 0 && py < self.height as i32 {
                            for x in 0..w {
                                let px = x_start + x;
                                if px >= 0 && px < self.width as i32 {
                                    let mut draw = true;
                                    let (mut dx, mut dy) = (0, 0);
                                    
                                    if x < rad && y < rad {
                                        dx = rad - x - 1;
                                        dy = rad - y - 1;
                                    } else if x >= w - rad && y < rad {
                                        dx = x - (w - rad);
                                        dy = rad - y - 1;
                                    } else if x < rad && y >= h - rad {
                                        dx = rad - x - 1;
                                        dy = y - (h - rad);
                                    } else if x >= w - rad && y >= h - rad {
                                        dx = x - (w - rad);
                                        dy = y - (h - rad);
                                    }
                                    
                                    if dx * dx + dy * dy >= rad * rad {
                                        draw = false;
                                    }

                                    if draw {
                                        self.buffer[(py * self.width as i32 + px) as usize] = *color;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(window) = &mut self.window {
            window
                .update_with_buffer(&self.buffer, self.width as usize, self.height as usize)
                .unwrap();

            // Frame pacing
            if self.target_fps > 0.0 {
                let target_dt = Duration::from_secs_f64(1.0 / self.target_fps);
                let elapsed = self.last_frame.elapsed();
                if elapsed < target_dt {
                    // Sleep for majority
                    let sleep_time = target_dt
                        .saturating_sub(elapsed)
                        .saturating_sub(Duration::from_millis(1));
                    if sleep_time > Duration::ZERO {
                        std::thread::sleep(sleep_time);
                    }
                    // Spin for remainder
                    while self.last_frame.elapsed() < target_dt {
                        std::hint::spin_loop();
                    }
                }
            }
        }
    }

    pub fn quit(&mut self) {
        self.window = None;
    }
}

pub fn vector2(x: f32, y: f32) -> Vector2 {
    Vector2::new(x, y)
}
pub fn sprite(path: String) -> Sprite {
    Sprite::new(path)
}
pub fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::new(x, y, width, height)
}
pub fn random(min: f32, max: f32) -> f32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    min + ((nanos as f32) / 1_000_000_000.0) * (max - min)
}
pub fn text(content: String) -> Text {
    Text::new(content)
}
#[flame(rename = "Game")]
pub fn game(title: String, width: u32, height: u32) -> Game {
    Game::new(title, width, height)
}
