extern crate amethyst;

use amethyst::{
    assets::Processor,
    core::transform::TransformBundle,
    ecs::{ReadExpect, Resources, SystemData},
    prelude::*,
    renderer::{
        pass::DrawFlat2DDesc, types::DefaultBackend, Factory, Format, GraphBuilder, GraphCreator,
        Kind, RenderGroupDesc, RenderingSystem, SpriteSheet, SubpassBuilder,
    },
    utils::application_root_dir,
    window::{ScreenDimensions, Window, WindowBundle},
};

mod pong;
use crate::pong::Pong;

fn main() -> amethyst::Result<()> {
    amethyst::start_logger(Default::default());

    let app_root = application_root_dir()?;
    let display_config_path = app_root.join("resources").join("display_config.ron");

    let game_data = GameDataBuilder::default()
        // The WindowBundle provides all the scaffolding for opening a window
        .with_bundle(WindowBundle::from_config_path(display_config_path))?
        // Add the transform bundle which handles tracking entity positions
        .with_bundle(TransformBundle::new())?
        // A Processor system is added to handle loading spritesheets.
        .with(
            Processor::<SpriteSheet>::new(),
            "sprite_sheet_processor",
            &[],
        )
        // The renderer must be executed on the same thread consecutively, so we
        // intialize it as thread_local which will always execute on the main thread.
        .with_thread_local(RenderingSystem::<DefaultBackend, _>::new(
            ExampleGraph::default(),
        ));

    let assets_dir = app_root.join("assets");
    let mut game = Application::new(assets_dir, Pong, game_data)?;
    game.run();

    Ok(())
}

// This graph structure is used for creating a proper `RenderGraph` for rendering.
// A renderGraph can be thought of as the stages during a render pass. In our case,
// we are only executing one subpass (DrawFlat2D, or the sprite pass). This graph
// also needs to be rebuilt whenever the window is resized, so the boilerplate code
// for that operation is also here.
#[derive(Default)]
struct ExampleGraph {
    dimensions: Option<ScreenDimensions>,
    dirty: bool,
}

impl GraphCreator<DefaultBackend> for ExampleGraph {
    // This trait method reports to the renderer if the graph must be rebuilt, usually
    // because the window has been resized. This implementation checks the screen size
    // and returns true if it has changed.
    fn rebuild(&mut self, res: &Resources) -> bool {
        // Rebuild when dimensions change, but wait until at least two frames have
        // the same.
        let new_dimensions = res.try_fetch::<ScreenDimensions>();
        use std::ops::Deref;
        if self.dimensions.as_ref() != new_dimensions.as_ref().map(|d| d.deref()) {
            self.dirty = true;
            self.dimensions = new_dimensions.map(|d| d.clone());
            return false;
        }
        return self.dirty;
    }

    // This is the core of a RenderGraph, which is building the actual graph with
    // subpasses and target images.
    fn builder(
        &mut self,
        factory: &mut Factory<DefaultBackend>,
        res: &Resources,
    ) -> GraphBuilder<DefaultBackend, Resources> {
        use amethyst::renderer::rendy::{
            graph::present::PresentNode,
            hal::command::{ClearDepthStencil, ClearValue},
        };

        self.dirty = true;

        // Retreive a reference to the target window, which is created by the
        // WindowBundle.
        let window = <ReadExpect<'_, Window>>::fetch(res);
        let dimensions = self.dimensions.as_ref().unwrap();
        let window_kind = Kind::D2(dimensions.width() as u32, dimensions.height() as u32, 1, 1);

        // Create a new drawing surface in our window.
        let surface = factory.create_surface(&window);
        let surface_format = factory.get_surface_format(&surface);

        // Begin building our RenderGraph
        let mut graph_builder = GraphBuilder::new();
        let color = graph_builder.create_image(
            window_kind,
            1,
            surface_format,
            // Clear screen to black
            Some(ClearValue::Color([0.0, 0.0, 0.0, 1.0].into())),
        );

        let depth = graph_builder.create_image(
            window_kind,
            1,
            Format::D32Sfloat,
            Some(ClearValue::DepthStencil(ClearDepthStencil(1.0, 0))),
        );

        // Create our single `Subpass`, which is the DrawFlat2D pass.
        // We pass the subpass builder a description of our pass for construction.
        let pass = graph_builder.add_node(
            SubpassBuilder::new()
                .with_group(DrawFlat2DDesc::new().builder())
                .with_color(color)
                .with_depth_stencil(depth)
                .into_pass(),
        );

        // Finally, add the pass to the graph.
        let _present = graph_builder
            .add_node(PresentNode::builder(factory, surface, color).with_dependency(pass));

        graph_builder
    }
}
