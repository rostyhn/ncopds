use image::DynamicImage;

use cursive::direction::Direction;
use cursive::event::EventResult;
use cursive::theme::{Color, ColorStyle};
use cursive::view::CannotFocus;
use cursive::Printer;
use cursive::Vec2;

/// stolen from https://github.com/lennart-finke/kakikun/blob/main/src/canvas.rs
/// Renders dynamic images inside a CanvasView

/// In memory representation of the content of the image
pub struct Board {
    pub size: Vec2,
    pub cells: Vec<Cell>,
}

impl Board {
    pub fn new(size: Vec2) -> Self {
        let n_cells = size.x * size.y;

        Board {
            size,
            cells: vec![
                Cell {
                    color: Color::Rgb(255, 255, 255),
                    backcolor: Color::Rgb(255, 255, 255),
                    symbol: ' '
                };
                n_cells
            ],
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct Cell {
    pub color: Color,
    pub backcolor: Color,
    pub symbol: char,
}

pub struct CanvasView {
    board: Board,
    overlay: Vec<Cell>,
}

impl CanvasView {
    pub fn new(size: Vec2) -> Self {
        let overlay = vec![
            Cell {
                color: Color::Rgb(255, 255, 255),
                backcolor: Color::Rgb(255, 255, 255),
                symbol: ' '
            };
            size.x * size.y
        ];
        let board = Board::new(size);

        CanvasView { board, overlay }
    }

    /// Sets the canvas to all white pixels.
    pub fn clear(&mut self) {
        self.overlay = vec![
            Cell {
                color: Color::Rgb(255, 255, 255),
                backcolor: Color::Rgb(255, 255, 255),
                symbol: ' '
            };
            self.board.size.x * self.board.size.y
        ]
    }

    /// Renders the dynamic image on the canvas view using ASCII characters.
    pub fn from_image(&mut self, img: &DynamicImage) {
        let mut overlay_new: Vec<Cell>;

        // don't like these hardcoded values...
        let rgbimg = DynamicImage::ImageRgb8(img.clone().into_rgb8())
            .thumbnail(50, 50)
            .into_rgb8();

        let (img_w, img_h) = rgbimg.dimensions() as (u32, u32);
        self.board = Board::new(Vec2::new(img_w as usize, (img_h / 2) as usize));

        self.clear(); //For quickly resizing the overlay

        overlay_new = vec![
            Cell {
                color: Color::Rgb(255, 255, 255),
                backcolor: Color::Rgb(255, 255, 255),
                symbol: ' '
            };
            self.board.size.x * self.board.size.y
        ];

        for (i, _cell) in self.overlay.iter().enumerate() {
            let x = (i % self.board.size.x) as u32;
            let y = (i / self.board.size.x) as u32;

            // Only every second line is parsed into the canvas to conserve image aspect ratio.
            let rgb = rgbimg.get_pixel(x, y * 2);
            overlay_new[i].backcolor = Color::Rgb(rgb[0], rgb[1], rgb[2]);
        }

        self.overlay = overlay_new;
    }
}

impl cursive::view::View for CanvasView {
    fn draw(&self, printer: &Printer) {
        for (i, cell) in self.overlay.iter().enumerate() {
            let x = i % self.board.size.x;
            let y = i / self.board.size.x;

            let text = cell.symbol;
            let backcolor = cell.backcolor;
            let color = cell.color;

            printer.with_color(ColorStyle::new(color, backcolor), |printer| {
                printer.print((x, y), &text.to_string())
            });
        }
    }

    fn take_focus(&mut self, _: Direction) -> Result<EventResult, CannotFocus> {
        Ok(EventResult::Consumed(None))
    }

    fn required_size(&mut self, _: Vec2) -> Vec2 {
        self.board.size.map_x(|x| x)
    }
}
