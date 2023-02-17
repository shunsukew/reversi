#![cfg_attr(not(feature = "std"), no_std)]

#[ink::contract]
mod reversi {
    use ink::{
        prelude::{
            vec,
            vec::Vec
        },
        prelude::string::String,
    };

    const ZERO_ADDRESS: [u8; 32] = [0; 32];

    const MAX_BOARD_SIZE: u8 = 10;
    const MIN_BOARD_SIZE: u8 = 6;

    #[derive(Clone, Copy, Debug, scale::Decode, scale::Encode, PartialEq, Eq)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub enum Disk {
        Black,
        White,
    }

    impl Disk {
        fn opposite(self) -> Self {
            match self {
                Self::White => Self::Black,
                Self::Black => Self::White,
            }
        }
    }

    #[derive(Clone, Debug, scale::Decode, scale::Encode)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct Board {
        // player_1: White, player_2: Black
        disks: Vec<Vec<Option<Disk>>>,
    }

    impl Board {
        fn new(size: u8) -> Self {
            assert!(size >= MIN_BOARD_SIZE && size <= MAX_BOARD_SIZE, "Board size is too big");
            assert!(size % 2 == 0, "Board size should be even number");

            let size = size as usize;
            let mut disks = vec![vec![None; size]; size];

            disks[size/2-1][size/2-1] = Some(Disk::White);
            disks[size/2][size/2] = Some(Disk::White);
            disks[size/2-1][size/2] = Some(Disk::Black);
            disks[size/2][size/2-1] = Some(Disk::Black);
            Self { disks }
        }
    }

    #[ink(storage)]
    pub struct Reversi {
        board_size: u8,
        players: [AccountId; 2],
        active_player_index: u8,
        board: Board,
        is_game_over: bool,
        winner: AccountId,
    }

    impl Reversi {
        #[ink(constructor)]
        pub fn new(board_size: u8, player_1: AccountId, player_2: AccountId) -> Self {
            assert!(player_1 != player_2, "palyer_1 and player_2 should be different account");

            Self {
                players: [player_1, player_2],
                // No random generator available so far.
                board_size,
                active_player_index: 0,
                board: Board::new(board_size),
                is_game_over: false,
                winner: ZERO_ADDRESS.into(),
            }
        }

        #[ink(message)]
        pub fn is_game_over(&self) -> bool {
            self.is_game_over
        }

        #[ink(message)]
        pub fn get_players(&self) -> [AccountId; 2] {
            self.players
        }

        #[ink(message)]
        pub fn get_active_player(&self) -> AccountId {
            self.players[self.active_player_index as usize]
        }

        #[ink(message)]
        pub fn is_active(&self, player: AccountId) -> bool {
            self.players[self.active_player_index as usize] == player
        }

        #[ink(message)]
        pub fn get_board(&self) -> Board {
            self.board.clone()
        }

        #[ink(message)]
        pub fn get_winner(&self) -> Result<AccountId, String> {
            if !self.is_game_over() {
                return Err(String::from("The game has not ended"))
            }
            Ok(self.winner)
        }

        #[ink(message)]
        pub fn make_move(&mut self, x: u8, y: u8) -> Result<(), String> {
            if self.is_game_over {
                return Err(String::from("The game has already ended"));
            }

            let player = Self::env().caller();
            if !self.is_active(player) {
                return Err(String::from("Invalid player"))
            }

            let disk = self.get_own_disk(player);
            self.place_disk(disk, x, y)?;


            // Opposite player can put disk? If yes, opponent's turn next.
            if self.can_place(disk.opposite()) {
                self.switch_active_player();
                return Ok(())
            }

            // Same player can put disk? If yes, same player's turn again.
            if self.can_place(disk) {
                return Ok(())
            }

            // Game is over, count disks and decide winner
            self.is_game_over = true;
            let (player_1_disk_count, player_2_disk_count) = self.count_disks();
            if player_1_disk_count > player_2_disk_count {
                self.winner = self.players[0];
            } else if player_1_disk_count < player_2_disk_count {
                self.winner = self.players[1];
            }

            Ok(())
        }

        // player_1 uses White disk, player_2 uses Black one.
        pub fn get_own_disk(&self, player: AccountId) -> Disk {
            if self.players[0] == player {
                return Disk::White;
            }
            Disk::Black
        }

        fn is_valid_place(&self, disk: Disk, x: u8, y: u8) -> bool {
            let x = x as i32;
            let y = y as i32;

            // outside of the board
            if !self.is_inside_board(x, y) {
                return false;
            }
            
            // A disk is already at x,y position
            if self.board.disks[y as usize][x as usize].is_some() {
                return false;
            }

            let is_valid_place = |disk: Disk, mut x: i32, mut y: i32, dx: i32, dy: i32| -> bool {
                // Check one next square.
                x += dx;
                y += dy;

                if !self.is_inside_board(x, y) {
                    return false;
                }

                match self.board.disks[y as usize][x as usize] {
                    Some(next_disk) => {
                        // Cannot place a disk if there's a same color disk at the next square.
                        if next_disk == disk {
                            return false;
                        }
                    },
                    None => {
                        // Cannot place a disk if the next square is blank.
                        return false;
                    }
                }

                x += dx;
                y += dy;

                while self.is_inside_board(x, y) {
                    match self.board.disks[y as usize][x as usize] {
                        Some(target_disk) => {
                            if target_disk == disk {
                                // Can place a disk if the same color disk is found.
                                // Because, this two pair of disks can sandwich opposite color disks.
                                return true;
                            }
                        }
                        None => {
                            // CAnnot place a disk if blank square is found.
                            return false
                        },
                    }

                    x += dx;
                    y += dy;
                }

                // pair disk to flip opponent's disks not found.
                false
            };

            // Check all 8 directions.
            if is_valid_place(disk, x, y, 1, 0) {
                return true;
            }
            if is_valid_place(disk, x, y, 0, 1) {
                return true;
            }
            if is_valid_place(disk, x, y, -1, 0) {
                return true;
            }
            if is_valid_place(disk, x, y, 0, -1) {
                return true;
            }
            if is_valid_place(disk, x, y, 1, 1) {
                return true;
            }
            if is_valid_place(disk, x, y, -1, -1) {
                return true;
            }
            if is_valid_place(disk, x, y, 1, -1) {
                return true;
            }
            if is_valid_place(disk, x, y, -1, 1) {
                return true;
            }

            false
        }

        fn place_disk(&mut self, disk: Disk, x: u8, y: u8) -> Result<(), String> {
            if !self.is_valid_place(disk, x, y) {
                return Err(String::from("Cannot place disk"));
            }

            // put disk at x,y position
            self.board.disks[y as usize][x as usize] = Some(disk);

            // flip opponent disks
            let mut flip_disks_in_direction = |disk: Disk, mut x: i32, mut y: i32, dx: i32, dy: i32| {
                x += dx;
                y += dy;

                while self.is_inside_board(x, y) {
                    if let Some(target_disk) = self.board.disks[y as usize][x as usize] {
                        if target_disk == disk {
                            break;
                        }

                        self.board.disks[y as usize][x as usize] = Some(disk);
                    }

                    x += dx;
                    y += dy;
                }
            };

            let x = x as i32;
            let y = y as i32;

            // flip disks in all 8 directions.
            flip_disks_in_direction(disk, x, y, 1, 0);
            flip_disks_in_direction(disk, x, y, 0, 1);
            flip_disks_in_direction(disk, x, y, -1, 0);
            flip_disks_in_direction(disk, x, y, 0, -1);
            flip_disks_in_direction(disk, x, y, 1, 1);
            flip_disks_in_direction(disk, x, y, -1, -1);
            flip_disks_in_direction(disk, x, y, 1, -1);
            flip_disks_in_direction(disk, x, y, -1, 1);

            Ok(())
        }

        fn can_place(&self, disk: Disk) -> bool {
            for i in 0..self.board_size {
                for j in 0..self.board_size {
                    if self.is_valid_place(disk, i, j) {
                        return true;
                    }
                }
            }
            false
        }

        fn is_inside_board(&self, x: i32, y: i32) -> bool {
            if x < 0
                || x >= self.board_size as i32
                || y < 0
                || y >= self.board_size as i32
            {
                return false;
            }
    
            true
        }

        fn switch_active_player(&mut self) {
            if self.active_player_index == 0 {
                self.active_player_index = 1;
                return;
            }
            self.active_player_index = 0;
        }
        
        fn count_disks(&self) -> (u8, u8) {
            let (mut white_counts, mut black_counts) = (0, 0);
            for i in 0..self.board_size as usize {
                for j in 0..self.board_size as usize {
                    if let Some(disk) = self.board.disks[j][i] {
                        match disk {
                            Disk::White => white_counts += 1,
                            Disk::Black => black_counts += 1,
                        }
                    }
                }
            }
            (white_counts, black_counts)
        }
    }

    #[cfg(test)]
    mod tests {
        use ink::env::test::default_accounts;

        use super::*;

        impl PartialEq for Board {
            fn eq(&self, other: &Board) -> bool {
                self.disks.iter().zip(other.disks.iter()).all(|(a,b)| a == b) 
            }
        }

        #[ink::test]
        fn constructor_works() {
            let default_accounts = default_accounts::<Environment>();
            let mut board_size : usize = 6;
            let reversi = Reversi::new(board_size as u8, default_accounts.alice, default_accounts.bob);
            assert_eq!(reversi.get_players(), [default_accounts.alice, default_accounts.bob]);

            assert_eq!(reversi.get_active_player(), default_accounts.alice);

            assert_eq!(reversi.is_active(default_accounts.alice), true);
            assert_eq!(reversi.is_active(default_accounts.bob), false);

            let board = Board {
                disks: vec![
                    vec![None; board_size],
                    vec![None; board_size],
                    vec![None, None, Some(Disk::White), Some(Disk::Black), None, None],
                    vec![None, None, Some(Disk::Black), Some(Disk::White), None, None],
                    vec![None; board_size],
                    vec![None; board_size],
                ]
            };
            assert_eq!(reversi.get_board(), board);

            board_size = 8;
            let reversi = Reversi::new(board_size as u8, default_accounts.alice, default_accounts.bob);
            let board = Board {
                disks: vec![
                    vec![None; board_size],
                    vec![None; board_size],
                    vec![None; board_size],
                    vec![None, None, None, Some(Disk::White), Some(Disk::Black), None, None, None],
                    vec![None, None, None, Some(Disk::Black), Some(Disk::White), None, None, None],
                    vec![None; board_size],
                    vec![None; board_size],
                    vec![None; board_size],
                ]
            };
            assert_eq!(reversi.get_board(), board);

            board_size = 10;
            let reversi = Reversi::new(board_size as u8, default_accounts.alice, default_accounts.bob);
            let board = Board {
                disks: vec![
                    vec![None; board_size],
                    vec![None; board_size],
                    vec![None; board_size],
                    vec![None; board_size],
                    vec![None, None, None, None, Some(Disk::White), Some(Disk::Black), None, None, None, None],
                    vec![None, None, None, None, Some(Disk::Black), Some(Disk::White), None, None, None, None],
                    vec![None; board_size],
                    vec![None; board_size],
                    vec![None; board_size],
                    vec![None; board_size],
                ]
            };
            assert_eq!(reversi.get_board(), board);
        }

        #[ink::test]
        fn is_valid_place_ok_1() {
            let default_accounts = default_accounts::<Environment>();
            let reversi = Reversi::new(6, default_accounts.alice, default_accounts.bob);

            //    0  1  2  3  4  5
            // 0
            // 1
            // 2 　　　　⚪️ ⚫️
            // 3        ⚫️ ⚪️
            // 4
            // 5
            // 
            assert!(reversi.is_valid_place(Disk::White, 3, 1));
            assert!(reversi.is_valid_place(Disk::White, 4, 2));
            assert!(reversi.is_valid_place(Disk::White, 1, 3));
            assert!(reversi.is_valid_place(Disk::White, 2, 4));
            assert_eq!(reversi.is_valid_place(Disk::White, 2, 1), false);
            assert_eq!(reversi.is_valid_place(Disk::White, 2, 2), false);
            assert_eq!(reversi.is_valid_place(Disk::White, 3, 2), false);
            assert_eq!(reversi.is_valid_place(Disk::White, 0, 0), false);
            assert_eq!(reversi.is_valid_place(Disk::White, 5, 6), false);

            assert!(reversi.is_valid_place(Disk::Black, 2, 1));
            assert!(reversi.is_valid_place(Disk::Black, 4, 3));
            assert!(reversi.is_valid_place(Disk::Black, 3, 4));
            assert!(reversi.is_valid_place(Disk::Black, 1, 2));
            assert_eq!(reversi.is_valid_place(Disk::Black, 3, 1), false);
            assert_eq!(reversi.is_valid_place(Disk::Black, 2, 2), false);
            assert_eq!(reversi.is_valid_place(Disk::Black, 3, 2), false);
            assert_eq!(reversi.is_valid_place(Disk::Black, 0, 0), false);
            assert_eq!(reversi.is_valid_place(Disk::Black, 6, 5), false);
        }

        #[ink::test]
        fn is_valid_place_ok_2() {
            let default_accounts = default_accounts::<Environment>();
            let reversi = Reversi {
                board_size: 6,
                players: [default_accounts.alice, default_accounts.bob],
                active_player_index: 0,
                winner: ZERO_ADDRESS.into(),
                is_game_over: false,
                board: Board {
                    disks: vec![
                        vec![None; 6],
                        vec![None, None, None, Some(Disk::White), None, None],
                        vec![None, None, Some(Disk::Black), Some(Disk::White), Some(Disk::Black), None],
                        vec![None, None, Some(Disk::Black), Some(Disk::White), None, None],
                        vec![None; 6],
                        vec![None; 6],
                    ],
                },
            };

            //    0  1  2  3  4  5
            // 0
            // 1          ⚪️
            // 2 　　　　⚫️ ⚪️ ⚫ ️ 
            // 3        ⚫️ ⚪️
            // 4
            // 5
            // 
            assert!(reversi.is_valid_place(Disk::White, 1, 2));
            assert!(reversi.is_valid_place(Disk::White, 1, 3));
            assert!(reversi.is_valid_place(Disk::White, 1, 4));
            assert!(reversi.is_valid_place(Disk::White, 5, 2));
            assert!(reversi.is_valid_place(Disk::White, 5, 3));
            assert_eq!(reversi.is_valid_place(Disk::White, 2, 1), false);
            assert_eq!(reversi.is_valid_place(Disk::White, 2, 2), false);
            assert_eq!(reversi.is_valid_place(Disk::White, 3, 2), false);
            assert_eq!(reversi.is_valid_place(Disk::White, 0, 0), false);
            assert_eq!(reversi.is_valid_place(Disk::White, 5, 6), false);

            assert!(reversi.is_valid_place(Disk::Black, 2, 0));
            assert!(reversi.is_valid_place(Disk::Black, 4, 0));
            assert!(reversi.is_valid_place(Disk::Black, 4, 4));
            assert_eq!(reversi.is_valid_place(Disk::Black, 2, 1), false);
            assert_eq!(reversi.is_valid_place(Disk::Black, 3, 0), false);
            assert_eq!(reversi.is_valid_place(Disk::Black, 3, 4), false);
        }
    }
}
