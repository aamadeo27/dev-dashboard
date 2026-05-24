// platform — OS-specific open operations

pub mod editor;
pub mod terminal;

pub use editor::lookup_project_path;
pub use editor::open_in_editor_impl;
pub use terminal::open_in_terminal_impl;
