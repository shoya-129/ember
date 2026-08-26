# Ember Engine

Ember is a high-performance, 60 FPS 2D game engine built natively in Rust and scripted entirely in **Flame** (`.fm`). 

## Features
- **Blazing Fast 2D Renderer**: Uses deferred rendering, integer-only blitting, and intelligent texture caching to easily maintain a solid 60 FPS even with hundreds of sprites.
- **Camera & Frustum Culling**: Built-in `Camera` support that automatically culls off-screen sprites, text, and rects to save CPU cycles.
- **Interactive UI Components**: Includes a fully functional `Button` UI component with bounds checking, click detection, perfectly centered text, and rounded corners!
- **Physics Framework**: A lightweight `PhysicsBody` wrapper for applying gravity and velocity to entities over time.
- **Keyboard & Mouse Input**: Polled input handling for keys (`Space`, `Up`, `Down`) and precise mouse coordinates/clicks.
- **Frame Pacing**: Integrated delta-time (`dt`) calculation and framerate targeting (`game.setTargetFPS(60.0)`).

> 🚧 **Note:** Audio modules and sound effects are currently under development.

## Getting Started

### The `Game` Annotation
Ember uses a global `@Game` annotation to initialize the window and inject the `game` object into your `main()` loop.

```flame
import ember

@Game(
    title = "My First Ember Game",
    width = 800,
    height = 600
)
fn main() {
    game.setTargetFPS(60.0)
    game.run()

    while game.running() {
        game.update()
        game.clear()
        
        // Update physics, check inputs, submit draw calls
        
        game.render()
    }

    game.quit()
}
```

### Rendering Sprites
Sprites are automatically cached upon loading so you can spawn hundreds of the same image without memory leaks.

```flame
let mut player = ember.sprite("hero.png")
player.setPosition(ember.vector2(100.0, 100.0))
player.setScale(ember.vector2(2.0, 2.0))

// In your game loop:
game.draw(&player)
```

### Physics Bodies
Wrap a sprite in a `PhysicsBody` to get instant gravity and velocity handling.

```flame
let mut phys = ember.PhysicsBody {
    sprite: ember.sprite("player.png"),
    velocity: ember.vector2(0.0, 0.0),
    gravity: 900.0
}

// In your game loop update:
phys.applyGravity(game.deltaTime())

if game.isKeyPressed("Space") {
    phys.jump(350.0) // Propels the sprite upwards
}
```

### UI Buttons
Create interactive buttons with beautifully rounded corners and auto-centered text.

```flame
let mut start_btn = ember.Button.new(
    140.0, 350.0, 200.0, 50.0, 
    "START", 
    50, 150, 255,   // Background (Blue)
    255, 255, 255   // Text (White)
)

// In your game loop update:
start_btn.onPress(game.mouseX(), game.mouseY(), game.isMouseDown(), () {
    print("Game Started!")
})

// In your game loop render:
let color = start_btn.bgR * 65536 + start_btn.bgG * 256 + start_btn.bgB
game.drawRoundedRect(&start_btn.rect, color, 12.0)
game.drawText(&start_btn.text)
```

## Playable Example: Flappy Bird
We've included a full, playable **Flappy Bird** clone inside the [`example`](./example) directory! It demonstrates:
- State management (`START`, `PLAYING`, `GAME_OVER`)
- Infinite scrolling pipes and procedural generation
- AABB bounding box collision detection
- Score tracking and UI rendering
- Interactive Start and Retry buttons

**To test the game:**
Navigate into the `example` directory and run the project using the Flame CLI.
