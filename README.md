# wgpu_renderer

This is a highly experimental 3d renderer written in rust, directly using wgpu. It features:

- Deferred rendering: a G-buffer stores albedo, normals and depth, with lighting handled in a separate pass. World-space positions are reconstructed from depth.
- Lighting and shadows: directional, point and spot lights, with shadow maps stored in an atlas and sampled using percentage-closer filtering.
- Instancing: instance data and batching for rendering repeated geometry.
- GPU resource management: infrastructure for storing and accessing GPU resources, alongside bind-group caching and pipeline management.
- Shader reflection: WGSL parsing through Naga to extract bindings, entry points and layout information, reducing some of the manual setup between shaders and Rust.
- ECS-based organisation: scene components and systems built around Shipyard.
- Render graph experimentation: dependency-based ordering of render work using a topological sort.

## What I was exploring

I found that in my prior renderer experiments, resource management was constantly a wall I kept hitting, especially around lifetimes, handling data dynamically and flexibly. I was aware that while native APIs 
like d3d12, vulkan supported bindless rendering, this was something not yet supported by wgpu. 

To work through this, I opted to try and use ECS as a mechanism to drive the entire renderer. By using components, entities and systems as a basis for handling the entire renderer from resource management, updates
and rendering, it led to some interesting effects with scheduling, data flow, connectivity between different types and the structure of a renderer.

It's led to the current codebase you see today. While the renderer works, it's not strictly the cleanest in dataflow, and
I learned a lot about what works, and what doesn't work. I probably wouldn't do this today, although it was a lot of fun to write after work on my Macbook in my hotel room until late at night :)

## Where things are 

Most of the renderer lives in src/renderer/.

ecs/ contains scene components and systems.

gpu_storage.rs and asset_manager.rs cover resource storage and management.

shader_reflect.rs contains the shader reflection code.

render_graph.rs contains the dependency graph implementation.

shadow_atlas.rs handles the shadow atlas.

assets/shaders/ contains the WGSL shaders.

 ## Status

This is a learning project, and the code should be read with that in mind. There are parts I'd approach differently now, but working through them was the point. 
It gave me a practical foundation to carry into further graphics and systems work. A lot of the principles I learned here directly informed my later work, and gave me a better
understanding of how renderers flow, the pitfalls and shortcomings of certain approaches, and passion for real bindless.
