use std::collections::HashMap;
use std::sync::Arc;
use petgraph::algo::toposort;
use petgraph::prelude::DiGraph;
use wgpu::{CommandEncoder, TextureView};
use crate::renderer::asset_manager::AssetManager;
use crate::renderer::ecs::camera_component::CameraComponent;
use crate::renderer::ecs::global_component::GlobalComponent;

/// A context that is passed to every render graph node so that it has access to the ECS-managed resources,
/// the wgpu encoder, and any additional state (e.g. camera, global uniforms).
pub struct RenderGraphContext<'a> {
    /// The command encoder (wrapped in an Option so we can temporarily take ownership).
    pub encoder: &'a mut Option<CommandEncoder>,
    /// The asset manager for fetching resources.
    pub asset_manager: &'a AssetManager,
    /// The global component that stores uniform buffers and other global state.
    pub global_component: &'a GlobalComponent,
    /// The current camera component.
    pub camera_component: &'a CameraComponent,
    /// A handle to the final output texture view.
    pub output_view: Arc<TextureView>,
    /// A handle to the shadow atlas view.
    pub shadow_atlas_view: Arc<TextureView>,
    // Additional fields (like ECS query views) can be added as needed.
}

/// A single node in the render graph.
pub struct RenderGraphNode<'a> {
    /// Unique name for the node.
    pub name: String,
    /// List of names of nodes this node depends on.
    pub dependencies: Vec<String>,
    /// The closure that executes this node.
    pub execute: Box<dyn Fn(&mut RenderGraphContext) + 'a>,
}

/// The render graph holds nodes and the dependency graph.
pub struct RenderGraph<'a> {
    /// Nodes stored by name.
    pub nodes: HashMap<String, RenderGraphNode<'a>>,
    /// Dependency graph for topological sorting.
    graph: DiGraph<String, ()>,
}

impl<'a> RenderGraph<'a> {
    /// Creates a new empty render graph.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            graph: DiGraph::new(),
        }
    }

    /// Adds a node to the render graph.
    pub fn add_node(&mut self, node: RenderGraphNode<'a>) {
        // Insert into the nodes map.
        self.nodes.insert(node.name.clone(), node);
    }

    /// Builds the dependency graph.
    pub fn build_dependency_graph(&mut self) {
        // Clear any previous graph.
        self.graph = DiGraph::new();
        // Map from node name to graph index.
        let mut indices = HashMap::new();
        // Add all nodes as vertices.
        for name in self.nodes.keys() {
            let idx = self.graph.add_node(name.clone());
            indices.insert(name.clone(), idx);
        }
        // Add edges for dependencies.
        for (name, node) in self.nodes.iter() {
            let source = indices.get(name).unwrap();
            for dep in &node.dependencies {
                if let Some(target) = indices.get(dep) {
                    // Create an edge: target must run before source.
                    self.graph.add_edge(*target, *source, ());
                }
            }
        }
    }

    /// Executes the render graph: sorts the nodes topologically and then runs each node’s closure.
    pub fn execute(&self, context: &mut RenderGraphContext) {
        // Get a sorted order of node names.
        let sorted = toposort(&self.graph, None).expect("Cyclic dependency in render graph");
        for node_name in sorted {
            // Retrieve the node.
            let name = &self.graph[node_name];
            if let Some(node) = self.nodes.get(name) {
                // Execute the node’s render pass.
                (node.execute)(context);
            }
        }
    }
}