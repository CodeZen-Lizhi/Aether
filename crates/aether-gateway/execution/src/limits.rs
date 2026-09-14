pub const MAX_ERROR_BODY_BYTES: usize = 16_384;
pub const MAX_STREAM_PREFETCH_FRAMES: usize = 5;
// Responses setup events can contain the full request configuration and be
// much larger than the first output event.  The prefetch buffer is only for
// classification; reaching it must never be treated as an upstream EOF.
pub const MAX_STREAM_PREFETCH_BYTES: usize = 256 * 1024;
