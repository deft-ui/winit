use crate::cursor::{BadImage, CursorImage};
use std::time::Duration;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PlatformCustomCursor;

#[derive(Debug)]
pub(crate) enum PlatformCustomCursorSource {
    Image(CursorImage),
    Url { url: String, hotspot_x: u16, hotspot_y: u16 },
    Animation { duration: Duration, cursors: Vec<crate::cursor::CustomCursor> },
}
#[derive(Debug)]
pub struct CustomCursorFuture {}

impl PlatformCustomCursorSource {
    pub fn from_rgba(
        rgba: Vec<u8>,
        width: u16,
        height: u16,
        hotspot_x: u16,
        hotspot_y: u16,
    ) -> Result<PlatformCustomCursorSource, BadImage> {
        Ok(PlatformCustomCursorSource::Image(CursorImage::from_rgba(
            rgba, width, height, hotspot_x, hotspot_y,
        )?))
    }
}
