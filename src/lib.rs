//! # Cursive
//!
//! Cursive is a TUI library built on top of ncurses-rs.
//! It allows to easily build layouts for text-based applications.
//!
//! ## Example
//! ```
//! extern crate cursive;
//!
//! use cursive::Cursive;
//! use cursive::view::TextView;
//!
//! fn main() {
//!     let mut siv = Cursive::new();
//!
//!     siv.add_layer(TextView::new("Hello World!\nPress q to quit."));
//!
//!     siv.add_global_callback('q' as i32, |s| s.quit());
//!
//!     siv.run();
//! }
//! ```
extern crate ncurses;
extern crate toml;

pub mod event;
pub mod view;
pub mod printer;
pub mod vec;
pub mod color;

mod div;

use std::any::Any;
use std::rc::Rc;
use std::collections::HashMap;

use vec::Vec2;
use view::View;
use printer::Printer;
use view::{StackView,Selector};

use event::{EventResult,Callback};

/// Identifies a screen in the cursive ROOT.
pub type ScreenId = usize;

/// Central part of the cursive library.
///
/// It initializes ncurses on creation and cleans up on drop.
/// To use it, you should populate it with views, layouts and callbacks,
/// then start the event loop with run().
///
/// It uses a list of screen, with one screen active at a time.
pub struct Cursive {
    screens: Vec<StackView>,

    active_screen: ScreenId,

    running: bool,

    global_callbacks: HashMap<i32, Rc<Callback>>,
}

impl Cursive {
    /// Creates a new Cursive root, and initialize ncurses.
    pub fn new() -> Self {
        ncurses::setlocale(ncurses::LcCategory::all, "");
        ncurses::initscr();
        ncurses::keypad(ncurses::stdscr, true);
        ncurses::noecho();
        ncurses::cbreak();
        ncurses::start_color();
        ncurses::curs_set(ncurses::CURSOR_VISIBILITY::CURSOR_INVISIBLE);
        color::load_legacy();
        // color::load_default();
        // color::load_theme("assets/style.toml").ok().unwrap();

        ncurses::wbkgd(ncurses::stdscr, ncurses::COLOR_PAIR(color::BACKGROUND));

        let mut res = Cursive {
            screens: Vec::new(),
            active_screen: 0,
            running: true,
            global_callbacks: HashMap::new(),
        };

        res.screens.push(StackView::new());

        res
    }

    /// Regularly redraws everything, even when no input is given. Between 0 and 1000.
    ///
    /// Call with fps=0 to disable (default value).
    pub fn set_fps(&self, fps: u32) {
        if fps == 0 {
            ncurses::timeout(-1);
        } else {
            ncurses::timeout(1000 / fps as i32);
        }
    }

    /// Returns a mutable reference to the currently active screen.
    pub fn screen_mut(&mut self) -> &mut StackView {
        let id = self.active_screen;
        self.screens.get_mut(id).unwrap()
    }

    /// Adds a new screen, and returns its ID.
    pub fn add_screen(&mut self) -> ScreenId {
        let res = self.screens.len();
        self.screens.push(StackView::new());
        res
    }

    /// Convenient method to create a new screen, and set it as active.
    pub fn add_active_screen(&mut self) -> ScreenId {
        let res = self.add_screen();
        self.set_screen(res);
        res
    }

    /// Sets the active screen. Panics if no such screen exist.
    pub fn set_screen(&mut self, screen_id: ScreenId) {
        if screen_id >= self.screens.len() {
            panic!("Tried to set an invalid screen ID: {}, but only {} screens present.", screen_id, self.screens.len());
        }
        self.active_screen = screen_id;
    }

    fn find_any(&mut self, selector: &Selector) -> Option<&mut Any> {
        // Internal find method that returns a Any object.
        self.screen_mut().find(selector)
    }

    /// Tries to find the view pointed to by the given path.
    /// If the view is not found, or if it is not of the asked type,
    /// it returns None.
    pub fn find<V: View + Any>(&mut self, selector: &Selector) -> Option<&mut V> {
        match self.find_any(selector) {
            None => None,
            Some(b) => b.downcast_mut::<V>(),
        }
    }

    /// Adds a global callback, triggered on the given key press when no view catches it.
    pub fn add_global_callback<F>(&mut self, key: i32, cb: F)
        where F: Fn(&mut Cursive) + 'static
    {
        self.global_callbacks.insert(key, Rc::new(Box::new(cb)));
    }

    /// Convenient method to add a layer to the current screen.
    pub fn add_layer<T: 'static + View>(&mut self, view: T) {
        self.screen_mut().add_layer(view);
    }

    // Handles a key event when it was ignored by the current view
    fn on_key_event(&mut self, ch: i32) {
        let cb = match self.global_callbacks.get(&ch) {
            None => return,
            Some(cb) => cb.clone(),
        };
        // Not from a view, so no viewpath here
        cb(self);
    }

    /// Returns the size of the screen, in characters.
    pub fn screen_size(&self) -> Vec2 {
        let mut x: i32 = 0;
        let mut y: i32 = 0;
        ncurses::getmaxyx(ncurses::stdscr, &mut y, &mut x);

        Vec2 {
            x: x as usize,
            y: y as usize,
        }
    }

    fn layout(&mut self) {
        let size = self.screen_size();
        self.screen_mut().layout(size);
    }

    fn draw(&mut self) {
        let printer = Printer {
            offset: Vec2::new(0,0),
            size: self.screen_size(),
        };
        self.screen_mut().draw(&printer, true);
        ncurses::refresh();
    }

    /// Runs the event loop.
    /// It will wait for user input (key presses) and trigger callbacks accordingly.
    /// Blocks until quit() is called.
    pub fn run(&mut self) {

        // And the big event loop begins!
        while self.running {
            // Do we need to redraw everytime?
            // Probably, actually.
            // TODO: Do we actually need to clear everytime?
            ncurses::clear();
            // TODO: Do we need to re-layout everytime?
            self.layout();
            // TODO: Do we need to redraw every view every time?
            // (Is this getting repetitive? :p)
            self.draw();

            // Blocks until the user press a key.
            // TODO: Add a timeout? Animations?
            let ch = ncurses::getch();

            // If the event was ignored, it is our turn to play with it.
            match self.screen_mut().on_key_event(ch) {
                EventResult::Ignored => self.on_key_event(ch),
                EventResult::Consumed(None) => (),
                EventResult::Consumed(Some(cb)) => cb(self),
            }
        }
    }

    /// Stops the event loop.
    pub fn quit(&mut self) {
        self.running = false;
    }
}

impl Drop for Cursive {
    fn drop(&mut self) {
        ncurses::endwin();
    }
}

