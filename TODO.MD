# Todos

[ ] Validate app mod.rs to be good builder code
[ ] Make this crate usable in foreign codebases easier

This is the idea I had; we abstract EVERYTHING into here, and then we can edit the GUI and other basics without having to worry about reworking old projects.

So like, in a new app we define our game models, any threads (aimbot, etc), then like

AppBuilder::new(process_name, window_name)
    .with_state(MyState)
    .with_logic(MyEsp)
    .with_aim(MyAimbot)
    .run()

and then it just goes, finds the process and the window n shi
