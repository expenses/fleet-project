use ultraviolet::Vec2;
use wgpu_glyph::ab_glyph::FontRef;

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

    pub fn start_section(&mut self, position: Vec2) {
        self.glyph_section.screen_position = position.into();
    }

    pub fn push(&mut self, args: std::fmt::Arguments, colour: [f32; 4]) {
        use std::fmt::Write;

        let start = self.cache_string.len();
        let _ = self.cache_string.write_fmt(args);
        let end = self.cache_string.len();

        let length = end - start;

        match self.lengths_and_colours.last_mut() {
            #[allow(clippy::float_cmp)]
            Some((last_length, last_colour)) if *last_colour == colour => {
                *last_length += length;
            }
            _ => {
                self.lengths_and_colours.push((length, colour));
            }
        }
    }

    pub fn finish_section(&mut self) {
        let mut offset = 0;

        for (length, colour) in &self.lengths_and_colours {
            let string = &self.cache_string[offset..offset + length];
            offset += length;

            // Use a transmute to change the lifetime of the string to be static.
            // This is VERY naughty but as far as I can tell is safe because the string
            // only needs to last until it is queued in the glyph brush.
            let string: &'static str = unsafe { std::mem::transmute(string) };
            self.glyph_section
                .text
                .push(wgpu_glyph::Text::new(string).with_color(*colour));
        }

        if !self.glyph_section.text.is_empty() {
            self.glyph_brush.queue(&self.glyph_section);
        }

        self.glyph_section.text.clear();
        self.lengths_and_colours.clear();
        self.cache_string.clear();
    }

    pub fn glyph_brush(&mut self) -> &mut wgpu_glyph::GlyphBrush<(), FontRef<'static>> {
        &mut self.glyph_brush
    }
}
