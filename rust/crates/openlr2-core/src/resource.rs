/// Opaque resource handle types.
///
/// These replace the bare `int` handles (`int GrHandle[]`, `int fontHandle[]`)
/// used throughout the C++ codebase. They carry no data — the actual resources
/// are managed by the runtime's resource table.

/// Handle to a loaded texture / image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureId(pub u32);

/// Handle to a loaded sound / audio buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SoundId(pub u32);

/// Handle to a loaded font.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontId(pub u32);
