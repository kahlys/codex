use std::io::{stdout, Write};
use std::thread::sleep;
use std::time::Duration;
use termion::{clear, cursor, terminal_size};

struct GameOfLife {
    grid: Vec<Vec<bool>>,
    rows: usize,
    cols: usize,
}

impl GameOfLife {
    fn new(rows: usize, cols: usize) -> GameOfLife {
        GameOfLife {
            grid: vec![vec![false; cols]; rows],
            rows,
            cols,
        }
    }

    fn next_generation(&mut self) {
        let mut new_grid = self.grid.clone();

        for (row, row_val) in self.grid.iter().enumerate().take(self.rows) {
            for (col, _col_val) in row_val.iter().enumerate().take(self.cols) {
                let alive_neighbours = self.count_alive_neighbours(row, col);
        
                new_grid[row][col] = match (self.grid[row][col], alive_neighbours) {
                    (true, x) if !(2..=3).contains(&x) => false,
                    (false, 3) => true,
                    (otherwise, _) => otherwise,
                };
            }
        }

        self.grid = new_grid;
    }

    fn count_alive_neighbours(&self, row: usize, col: usize) -> usize {
        let mut count = 0;

        for i in -1..=1 {
            for j in -1..=1 {
                if i != 0 || j != 0 {
                    let neighbour_row = (row as isize + i).rem_euclid(self.rows as isize) as usize;
                    let neighbour_col = (col as isize + j).rem_euclid(self.cols as isize) as usize;

                    if self.grid[neighbour_row][neighbour_col] {
                        count += 1;
                    }
                }
            }
        }

        count
    }

    fn spawn_glider(&mut self, top_left_row: usize, top_left_col: usize) {
        let glider = vec![
            vec![false, true, false],
            vec![false, false, true],
            vec![true, true, true],
        ];

        self.spawn_structure(top_left_row, top_left_col, glider);
    }

    fn spawn_lightweight_spaceship(&mut self, top_left_row: usize, top_left_col: usize) {
        let lwss = vec![
            vec![false, true, true, true, true],
            vec![true, false, false, false, true],
            vec![false, false, false, false, true],
            vec![true, false, false, true, false],
        ];

        self.spawn_structure(top_left_row, top_left_col, lwss);
    }

    fn spawn_structure(
        &mut self,
        top_left_row: usize,
        top_left_col: usize,
        structure: Vec<Vec<bool>>,
    ) {
        for (i, row) in structure.iter().enumerate() {
            for (j, &cell) in row.iter().enumerate() {
                let row = top_left_row + i;
                let col = top_left_col + j;

                if row < self.rows && col < self.cols {
                    self.grid[row][col] = cell;
                }
            }
        }
    }

    fn print(&self) {
        print!("{}", clear::All);
        print!("{}", cursor::Goto(1, 1));
        print!("{}", cursor::Hide);

        let stdout = stdout();
        let mut handle = stdout.lock();

        for row in &self.grid {
            for &cell in row {
                write!(handle, "{}", if cell { 'o' } else { ' ' }).unwrap();
            }
            writeln!(handle).unwrap();
        }

        handle.flush().unwrap();
        print!("{}", cursor::Show);
    }
}

fn main() {
    let (terminal_cols, terminal_rows) = terminal_size().unwrap();
    let rows = terminal_rows as usize - 5;
    let cols = terminal_cols as usize - 5;

    let mut game = GameOfLife::new(rows, cols);

    game.spawn_glider(1, 1);
    game.spawn_lightweight_spaceship(10, 10);

    loop {
        game.print();
        game.next_generation();
        sleep(Duration::from_millis(50));
    }
}
