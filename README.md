# wgpu_renderer

This is a highly experimental 3D renderer written in Rust, directly using wgpu. It features:

* Deferred rendering: a G-buffer stores albedo, normals and depth, with lighting handled in a separate pass. World-space positions are reconstructed from depth.
* Lighting and shadows: directional, point and spot lights, with shadow maps stored in an atlas and sampled using percentage-closer filtering.
* Instancing: instance data and batching for rendering repeated geometry.
* GPU resource management: infrastructure for storing and accessing GPU resources, alongside bind-group caching and pipeline management.
* Shader reflection: WGSL parsing through Naga to extract bindings, entry points and layout information, reducing some of the manual setup between shaders and Rust.
* ECS-based organisation: scene components and systems built around Shipyard.
* Render graph experimentation: dependency-based ordering of render work using a topological sort.

## What I was exploring

In my prior renderer experiments, resource management was constantly a wall I kept hitting, especially around lifetimes and handling data dynamically and flexibly. I was interested in bindless approaches in Vulkan and D3D12, but hadn’t found a way to get the resource model I wanted with wgpu.

To work through this, I opted to try using ECS to drive the entire renderer. Using components, entities and systems for resource management, updates and rendering led to some interesting effects on scheduling, data flow, connectivity between different types and the structure of the renderer.

That led to the codebase you see today. While the renderer works, the data flow isn't the cleanest, and I learned a lot about what works and what doesn't. I probably wouldn’t structure the whole renderer this way today, although it was a lot of fun to write after work on my MacBook in my hotel room until late at night :)

## Where things are

Most of the renderer lives in `src/renderer/`.

* `ecs/` contains scene components and systems.
* `gpu_storage.rs` and `asset_manager.rs` cover resource storage and management.
* `shader_reflect.rs` contains the shader reflection code.
* `render_graph.rs` contains the dependency graph implementation.
* `shadow_atlas.rs` handles the shadow atlas.
* Shaders live in `assets/shaders/` at the repository root.

## Status

This is a learning project, and the code should be read with that in mind. There are parts I'd approach differently now, but working through them was the point.

It gave me a practical foundation to carry into further graphics and systems work. A lot of the principles I learned here directly informed my later work, gave me a better understanding of how renderers fit together and the pitfalls of certain approaches, and left me wanting to explore bindless properly.
