use ultraviolet::Vec2;
use wgpu_glyph::ab_glyph::{FontRef, PxScale};

pub struct GlyphLayoutCache {
    glyph_brush: wgpu_glyph::GlyphBrush<(), FontRef<'static>>,
    cache_string: String,
    lengths_and_colours: Vec<(usize, [f32; 4])>,
    glyph_section: wgpu_glyph::Section<'static, wgpu_glyph::Extra>,
}

impl GlyphLayoutCache {
    pub fn new(glyph_brush: wgpu_glyph::GlyphBrush<(), FontRef<'static>>) -> Self {
        Self {
            glyph_brush,
            cache_string: Default::default(),
            lengths_and_colours: Default::default(),
            glyph_section: Default::default(),
        }
    }

    pub fn start_section(&mut self, position: Vec2, dpi_factor: f32) -> GlyphBrushSection {
        self.glyph_section.screen_position = position.into();

        GlyphBrushSection {
            inner: self,
            scale: PxScale::from(16.0 * dpi_factor),
        }
    }

    pub fn glyph_brush(&mut self) -> &mut wgpu_glyph::GlyphBrush<(), FontRef<'static>> {
        &mut self.glyph_brush
    }
}

pub struct GlyphBrushSection<'a> {
    inner: &'a mut GlyphLayoutCache,
    scale: PxScale,
}

impl<'a> GlyphBrushSection<'a> {
    pub fn push(&mut self, args: std::fmt::Arguments, colour: [f32; 4]) {
        use std::fmt::Write;

        let start = self.inner.cache_string.len();
        let _ = self.inner.cache_string.write_fmt(args);
        let end = self.inner.cache_string.len();
        let length = end - start;

        match self.inner.lengths_and_colours.last_mut() {
            #[allow(clippy::float_cmp)]
            Some((last_length, last_colour)) if *last_colour == colour => {
                *last_length += length;
            }
            _ => {
                self.inner.lengths_and_colours.push((length, colour));
            }
        }
    }
}

// I'm a slut for RAII
impl<'a> Drop for GlyphBrushSection<'a> {
    fn drop(&mut self) {
        let mut offset = 0;

        for (length, colour) in &self.inner.lengths_and_colours {
            let string = &self.inner.cache_string[offset..offset + length];
            offset += length;

            // Use a transmute to change the lifetime of the string to be static.
            // This is VERY naughty but as far as I can tell is safe because the string
            // only needs to last until it is queued in the glyph brush.
            let string: &'static str = unsafe { std::mem::transmute::<_, &str>(string) };
            self.inner.glyph_section.text.push(
                wgpu_glyph::Text::new(string)
                    .with_scale(self.scale)
                    .with_color(*colour),
            );
        }

        if !self.inner.glyph_section.text.is_empty() {
            self.inner.glyph_brush.queue(&self.inner.glyph_section);
        }

        self.inner.glyph_section.text.clear();
        self.inner.lengths_and_colours.clear();
        self.inner.cache_string.clear();
    }
}
