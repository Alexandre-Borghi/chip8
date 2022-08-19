use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::sys::KeyCode;
use std::time::Duration;

// ╔═══╦═══╦═══╦═══╗
// ║ 1 ║ 2 ║ 3 ║ C ║
// ╠═══╬═══╬═══╬═══╣
// ║ 4 ║ 5 ║ 6 ║ D ║
// ╠═══╬═══╬═══╬═══╣
// ║ 7 ║ 8 ║ 9 ║ E ║
// ╠═══╬═══╬═══╬═══╣
// ║ A ║ 0 ║ B ║ F ║
// ╚═══╩═══╩═══╩═══╝
const KEYS: [Keycode; 16] = [
    Keycode::X,    // 0
    Keycode::Num1, // 1
    Keycode::Num2, // 2
    Keycode::Num3, // 3
    Keycode::Q,    // 4
    Keycode::W,    // 5
    Keycode::E,    // 6
    Keycode::A,    // 7
    Keycode::S,    // 8
    Keycode::D,    // 9
    Keycode::Z,    // A
    Keycode::C,    // B
    Keycode::Num4, // C
    Keycode::R,    // D
    Keycode::F,    // E
    Keycode::V,    // F
];

fn main() {
    let mut chip8 = Chip8::new();
    chip8.load_cartridge(include_bytes!("../files/chip8-test-suite.ch8"));

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("CHIP-8 Emulator", SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();
    'running: loop {
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();

        let mut frame_keys: [Option<KeyState>; 16] = Default::default();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                _ => {}
            }

            if let Event::KeyDown {
                keycode: Some(key), ..
            } = event
            {
                for (i, keycode) in KEYS.iter().enumerate() {
                    if *keycode == key {
                        frame_keys[i] = Some(KeyState::Pressed);
                        break;
                    }
                }
            }

            if let Event::KeyUp {
                keycode: Some(key), ..
            } = event
            {
                for (i, keycode) in KEYS.iter().enumerate() {
                    if *keycode == key {
                        frame_keys[i] = Some(KeyState::Released);
                        break;
                    }
                }
            }
        }

        chip8.set_keys(frame_keys);
        chip8.step();

        println!("Screen: {:?}", chip8.screen);

        // Draw screen
        for y in 0..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                const COLORS: [Color; 2] = [Color::BLACK, Color::WHITE];
                let idx = x + y * SCREEN_WIDTH;
                let col = COLORS[chip8.screen[idx] as usize];
                canvas.set_draw_color(col);
                canvas.draw_point((x as i32, y as i32)).unwrap();
            }
        }

        canvas.present();
        std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}

const RAM_SIZE: usize = 4096;
const STACK_SIZE: usize = 12;

const SCREEN_WIDTH: usize = 64;
const SCREEN_HEIGHT: usize = 32;
const SCREEN_BUF_SIZE: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

const OP_LENGTH: u16 = 2;

const KEYS_COUNT: usize = 16;

const FONT_SPRITES_ADDR: u16 = 0x0000;
const FONT_SPRITE_SIZE: u16 = 5;

const CARTRIDGE_START_ADDR: u16 = 0x200;

#[allow(dead_code)]
pub struct Chip8 {
    /// Program counter
    pc: u16,

    /// General-purpose registers (V0 -> VF)
    registers: [u8; 16],

    /// "I" address register
    register_i: u16,

    /// Delay timer register
    register_delay: u8,

    /// Milliseconds to the next decrement of the delay timer
    delay_timer: f32,

    /// Sound timer register
    register_sound: u8,

    /// Milliseconds to the next decrement of the sound timer
    sound_timer: f32,

    /// RAM
    ram: [u8; RAM_SIZE],

    /// Stack
    stack: [u16; STACK_SIZE],

    /// Stack pointer, index in the stack
    sp: usize,

    /// Screen buffer
    pub screen: [u8; SCREEN_BUF_SIZE / 8],

    /// Keys states
    keys: [bool; KEYS_COUNT],

    /// The last instruction was `FX0A` and no key has been pressed yet
    waiting_for_key: bool,

    /// Register that receives the result from waiting for a key (`FX0A`)
    waiting_for_key_vx: u8,
}

impl Chip8 {
    pub const fn new() -> Self {
        Self {
            pc: CARTRIDGE_START_ADDR,
            registers: [0; 16],
            register_i: 0,
            register_delay: 0,
            delay_timer: 0.0,
            register_sound: 0,
            sound_timer: 0.0,
            ram: [0; RAM_SIZE],
            stack: [0; STACK_SIZE],
            sp: 0,
            screen: [0; SCREEN_BUF_SIZE / 8],
            keys: [false; KEYS_COUNT],
            waiting_for_key: false,
            waiting_for_key_vx: 0,
        }
    }

    pub fn load_cartridge(&mut self, rom: &[u8]) {
        for (i, b) in rom.iter().enumerate() {
            self.ram[CARTRIDGE_START_ADDR as usize + i] = *b;
        }
    }

    /// Execute the next instruction
    pub fn step(&mut self) {
        if self.waiting_for_key {
            return;
        }

        // TODO: Take dt as an argument and decrement the timer correctly
        // For now, we just call step() 60 times per second
        self.register_delay = self.register_delay.saturating_sub(1);
        self.register_sound = self.register_sound.saturating_sub(1);

        let op_hi = self.ram[self.pc as usize];
        let op_lo = self.ram[self.pc as usize + 1];
        let op = ((op_hi as u16) << 8) | op_lo as u16;
        match (op & 0xF000) >> 12 {
            0x0 => self.exec_0(op),
            0x1 => self.exec_1(op),
            0x2 => self.exec_2(op),
            0x3 => self.exec_3(op),
            0x4 => self.exec_4(op),
            0x5 => self.exec_5(op),
            0x6 => self.exec_6(op),
            0x7 => self.exec_7(op),
            0x8 => self.exec_8(op),
            0x9 => self.exec_9(op),
            0xA => self.exec_A(op),
            0xB => self.exec_B(op),
            0xC => self.exec_C(op),
            0xD => self.exec_D(op),
            0xE => self.exec_E(op),
            0xF => self.exec_F(op),
            _ => unreachable!(),
        }
    }

    pub fn set_keys(&mut self, keys: [Option<KeyState>; KEYS_COUNT]) {
        let mut last_key_pressed: Option<u8> = None;
        for (i, k) in keys.iter().enumerate() {
            match k {
                Some(KeyState::Pressed) => {
                    self.keys[i] = true;
                    last_key_pressed = Some(i as u8);
                }
                Some(KeyState::Released) => self.keys[i] = false,
                None => {}
            };
        }

        if let Some(key) = last_key_pressed {
            if self.waiting_for_key {
                println!("Got key !");
                self.waiting_for_key = false;
                self.registers[self.waiting_for_key_vx as usize] = key;
            }
        }
    }

    /// op: `0NNN`
    /// Execute machine language subroutine at address `NNN`
    fn exec_0(&mut self, op: u16) {
        const CLEAR_SCREEN: u16 = 0x0E0;
        const RET_FROM_SUB: u16 = 0x0EE;

        let address = op & 0x0FFF;
        match address {
            CLEAR_SCREEN => self.clear_screen(),
            RET_FROM_SUB => self.ret_from_sub(),
            _ => unimplemented!(
                "No machine language subroutine at address 0x{:04X} (pc = 0x{:04X})",
                address,
                self.pc
            ),
        }
        self.pc += OP_LENGTH;
    }

    /// Clear the screen to 0
    fn clear_screen(&mut self) {
        self.screen = [0; SCREEN_BUF_SIZE / 8];
    }

    /// Return from a subroutine
    fn ret_from_sub(&mut self) {
        self.sp -= 1;
        self.pc = self.stack[self.sp];
    }

    /// op: 1NNN
    /// Jump to address NNN
    fn exec_1(&mut self, op: u16) {
        let address = op & 0x0FFF;
        self.pc = address;
    }

    /// op: 2NNN
    /// Execute subroutine starting at address NNN
    fn exec_2(&mut self, op: u16) {
        let address = op & 0x0FFF;
        self.stack[self.sp] = self.pc;
        self.sp += 1;
        self.pc = address;
    }

    /// op: 3XNN
    /// Skip the following instruction if the value of register VX equals NN
    fn exec_3(&mut self, op: u16) {
        let [vx, nn] = (op & 0x0FFF).to_be_bytes();
        if self.registers[vx as usize] == nn {
            self.pc += OP_LENGTH * 2;
        } else {
            self.pc += OP_LENGTH;
        }
    }

    /// op: 4XNN
    /// Skip the following instruction if the value of register VX is not equal to NN
    fn exec_4(&mut self, op: u16) {
        let [vx, nn] = (op & 0x0FFF).to_be_bytes();
        if self.registers[vx as usize] != nn {
            self.pc += OP_LENGTH * 2;
        } else {
            self.pc += OP_LENGTH;
        }
    }

    /// op: 5XY0
    /// Skip the following instruction if the value of register VX is equal to the value of register VY
    fn exec_5(&mut self, op: u16) {
        assert_eq!(
            0,
            op & 0x000F,
            "5### instruction should end with a 0, but is actually 0x{:04X} (pc = 0x{:04X})",
            op,
            self.pc
        );

        let vx = (op & 0x0F00) >> 8;
        let vy = (op & 0x00F0) >> 4;

        if self.registers[vx as usize] == self.registers[vy as usize] {
            self.pc += OP_LENGTH * 2;
        } else {
            self.pc += OP_LENGTH;
        }
    }

    /// op: 6XNN
    /// Store number NN in register VX
    fn exec_6(&mut self, op: u16) {
        let [vx, nn] = (op & 0x0FFF).to_be_bytes();
        self.registers[vx as usize] = nn;
        self.pc += OP_LENGTH;
    }

    /// op: 7XNN
    /// Add the value NN to register VX
    fn exec_7(&mut self, op: u16) {
        let [vx, nn] = (op & 0x0FFF).to_be_bytes();
        (self.registers[vx as usize], _) = self.registers[vx as usize].overflowing_add(nn);
        self.pc += OP_LENGTH;
    }

    /// op: 8XYS
    /// Store/Do math
    fn exec_8(&mut self, op: u16) {
        const STORE_VY_IN_VX: u16 = 0x0;
        const VX_OR_VY: u16 = 0x1;
        const VX_AND_VY: u16 = 0x2;
        const VX_XOR_VY: u16 = 0x3;
        const ADD_VY_TO_VX: u16 = 0x4;
        const SUB_VY_FROM_VX: u16 = 0x5;
        const RSH_VY_TO_VX: u16 = 0x6;
        const VY_MINUS_VX: u16 = 0x7;
        const LSH_VY_TO_VX: u16 = 0xE;

        let suffix = op & 0x000F;
        match suffix {
            STORE_VY_IN_VX => self.store_vy_in_vx(op),
            VX_OR_VY => self.vx_or_vy(op),
            VX_AND_VY => self.vx_and_vy(op),
            VX_XOR_VY => self.vx_xor_vy(op),
            ADD_VY_TO_VX => self.add_vy_to_vx(op),
            SUB_VY_FROM_VX => self.sub_vy_from_vx(op),
            RSH_VY_TO_VX => self.rsh_vy_to_vx(op),
            VY_MINUS_VX => self.vy_minus_vx(op),
            LSH_VY_TO_VX => self.lsh_vy_to_vx(op),
            _ => unreachable!(
                "8### instruction: invalid suffix: instruction is 0x{:04X} (pc = 0x{:04X}",
                op, self.pc
            ),
        }
    }

    /// op: `8XY0`
    /// Store the value of register `VY` in register `VX`
    fn store_vy_in_vx(&mut self, op: u16) {
        let vx = op & 0x0F00 >> 8;
        let vy = op & 0x00F0 >> 4;
        self.registers[vx as usize] = self.registers[vy as usize];
        self.pc += OP_LENGTH;
    }

    /// op: `8XY1`
    /// Set `VX` to `VX` OR `VY`
    fn vx_or_vy(&mut self, op: u16) {
        let vx = op & 0x0F00 >> 8;
        let vy = op & 0x00F0 >> 4;
        self.registers[vx as usize] |= self.registers[vy as usize];
        self.pc += OP_LENGTH;
    }

    /// op: `8XY2`
    /// Set `VX` to `VX` AND `VY`
    fn vx_and_vy(&mut self, op: u16) {
        let vx = op & 0x0F00 >> 8;
        let vy = op & 0x00F0 >> 4;
        self.registers[vx as usize] &= self.registers[vy as usize];
        self.pc += OP_LENGTH;
    }

    /// op: `8XY3`
    /// Set `VX` to `VX` XOR `VY`
    fn vx_xor_vy(&mut self, op: u16) {
        let vx = op & 0x0F00 >> 8;
        let vy = op & 0x00F0 >> 4;
        self.registers[vx as usize] ^= self.registers[vy as usize];
        self.pc += OP_LENGTH;
    }

    /// op: `8XY4`
    /// Add the value of register `VY` to register `VX`<br>
    /// Set `VF` to `01` if a carry occurs<br>
    /// Set `VF` to `00` if a carry does not occur
    fn add_vy_to_vx(&mut self, op: u16) {
        let vx = (op & 0x0F00 >> 8) as usize;
        let vy = (op & 0x00F0 >> 4) as usize;
        let (res, carry) = self.registers[vx].overflowing_add(self.registers[vy]);
        self.registers[vx] = res;
        self.registers[0xF] = carry as u8;
        self.pc += OP_LENGTH;
    }

    /// op: `8XY5`
    /// Subtract the value of register `VY` from register `VX`<br>
    /// Set `VF` to `00` if a borrow occurs<br>
    /// Set `VF` to `01` if a borrow does not occur
    fn sub_vy_from_vx(&mut self, op: u16) {
        let vx = (op & 0x0F00 >> 8) as usize;
        let vy = (op & 0x00F0 >> 4) as usize;
        let (res, borrow) = self.registers[vx].overflowing_sub(self.registers[vy]);
        self.registers[vx] = res;
        self.registers[0xF] = borrow as u8;
        self.pc += OP_LENGTH;
    }

    /// op: `8XY6`
    /// Store the value of register `VY` shifted right one bit in register `VX`<br>
    /// Set register `VF` to the least significant bit prior to the shift<br>
    /// `VY` is unchanged
    fn rsh_vy_to_vx(&mut self, op: u16) {
        let vx = (op & 0x0F00 >> 8) as usize;
        let vy = (op & 0x00F0 >> 4) as usize;
        self.registers[0xF] = self.registers[vy] & 0x01;
        self.registers[vx] = self.registers[vy] >> 1;
        self.pc += OP_LENGTH;
    }

    /// op: `8XY7`
    /// Set register `VX` to the value of `VY` minus `VX`<br>
    /// Set `VF` to `00` if a borrow occurs<br>
    /// Set `VF` to `01` if a borrow does not occur
    fn vy_minus_vx(&mut self, op: u16) {
        let vx = (op & 0x0F00 >> 8) as usize;
        let vy = (op & 0x00F0 >> 4) as usize;
        let (res, borrow) = self.registers[vy].overflowing_sub(self.registers[vx]);
        self.registers[vx] = res;
        self.registers[0xF] = borrow as u8;
        self.pc += OP_LENGTH;
    }

    /// op: `8XYE`
    /// Store the value of register `VY` shifted left one bit in register `VX`<br>
    /// Set register `VF` to the most significant bit prior to the shift<br>
    /// `VY` is unchanged
    fn lsh_vy_to_vx(&mut self, op: u16) {
        let vx = (op & 0x0F00 >> 8) as usize;
        let vy = (op & 0x00F0 >> 4) as usize;
        self.registers[0xF] = self.registers[vy] & 0x80 >> 7;
        self.registers[vx] = self.registers[vy] << 1;
        self.pc += OP_LENGTH;
    }

    /// op: `9XY0`
    /// Skip the following instruction if the value of register VX is not equal to the value of register VY
    fn exec_9(&mut self, op: u16) {
        assert_eq!(
            0,
            op & 0x000F,
            "9### instruction should end with a 0, but is actually 0x{:04X} (pc = 0x{:04X})",
            op,
            self.pc
        );

        let vx = (op & 0x0F00) >> 8;
        let vy = (op & 0x00F0) >> 4;

        if self.registers[vx as usize] != self.registers[vy as usize] {
            self.pc += OP_LENGTH * 2;
        } else {
            self.pc += OP_LENGTH;
        }
    }

    /// op: `ANNN`
    /// Store memory address NNN in register I
    fn exec_A(&mut self, op: u16) {
        let address = op & 0x0FFF;
        self.register_i = address;
        self.pc += OP_LENGTH;
    }

    /// op: `BNNN`
    /// Jump to address NNN + V0
    fn exec_B(&mut self, op: u16) {
        let address = op & 0x0FFF;
        self.pc = address + self.registers[0x0] as u16;
    }

    /// op: `CXNN`
    /// Set VX to a random number with a mask of NN
    fn exec_C(&mut self, op: u16) {
        let [vx, nn] = (op & 0x0FFF).to_be_bytes();
        self.registers[vx as usize] = rand::random::<u8>() & nn;
        self.pc += OP_LENGTH;
    }

    /// op: `DXYN`
    /// Draw a sprite at position `VX`, `VY` with `N` bytes of sprite data starting at the address stored in `I`<br>
    /// Set `VF` to `01` if any set pixels are changed to unset, and `00` otherwise
    fn exec_D(&mut self, op: u16) {
        const SPRITE_WIDTH: u8 = 8;

        let vx = (op & 0x0F00) >> 8;
        let vy = (op & 0x00F0) >> 4;
        let n = (op & 0x000F) as u8;
        let x = self.registers[vx as usize] % SCREEN_WIDTH as u8;
        let y = self.registers[vy as usize] % SCREEN_HEIGHT as u8;
        let w = SPRITE_WIDTH - ((x + SPRITE_WIDTH) as i8 - SCREEN_WIDTH as i8).max(0) as u8;
        let h = n - ((y + n) as i8 - SCREEN_HEIGHT as i8).max(0) as u8;

        println!("Drawing sprite at ({x}, {y}) of size ({w}, {h})");

        for i in 0..h {
            for j in 0..w {
                let x = x + j;
                let y = y + i;
                let sprite_pos = i as u16 + j as u16 * SPRITE_WIDTH as u16;
                let ram_pos = sprite_pos + self.register_i;
                let screen_pos = x as usize + y as usize * SCREEN_WIDTH;
                self.screen[screen_pos] ^= self.ram[ram_pos as usize];
            }
        }

        println!();

        self.pc += OP_LENGTH;
    }

    /// op: `EXSS`
    /// Skip instruction depending on key
    fn exec_E(&mut self, op: u16) {
        let suffix = op & 0x00FF;
        match suffix {
            0x9E => self.skip_if_key_pressed(op),
            0xA1 => self.skip_if_key_not_pressed(op),
            _ => unreachable!(
                "E### instruction: invalid suffix: instruction is 0x{:04X} (pc = 0x{:04X}",
                op, self.pc
            ),
        }
    }

    /// op: `EX9E`
    /// Skip the following instruction if the key corresponding to the hex value currently stored in register `VX` is pressed
    fn skip_if_key_pressed(&mut self, op: u16) {
        let vx = (op & 0x0F00) >> 8;
        let key = self.registers[vx as usize];
        if self.keys[key as usize] {
            self.pc += OP_LENGTH * 2;
        } else {
            self.pc += OP_LENGTH;
        }
    }

    /// op: `EXA1`
    /// Skip the following instruction if the key corresponding to the hex value currently stored in register `VX` is not pressed
    fn skip_if_key_not_pressed(&mut self, op: u16) {
        let vx = (op & 0x0F00) >> 8;
        let key = self.registers[vx as usize];
        if !self.keys[key as usize] {
            self.pc += OP_LENGTH * 2;
        } else {
            self.pc += OP_LENGTH;
        }
    }

    /// op: `FXSS`
    /// Misc
    fn exec_F(&mut self, op: u16) {
        const STORE_DELAY: u8 = 0x07;
        const WAIT_FOR_KEY: u8 = 0x0A;
        const SET_DELAY: u8 = 0x15;
        const SET_SOUND: u8 = 0x18;
        const ADD_VX_TO_I: u8 = 0x1E;
        const SET_I_TO_FONT: u8 = 0x29;
        const STORE_BCD: u8 = 0x33;
        const STORE_REGISTERS: u8 = 0x55;
        const RESTORE_REGISTERS: u8 = 0x65;

        let suffix = op.to_be_bytes()[1];
        match suffix {
            STORE_DELAY => self.store_delay(op),
            WAIT_FOR_KEY => self.wait_for_key(op),
            SET_DELAY => self.set_delay(op),
            SET_SOUND => self.set_sound(op),
            ADD_VX_TO_I => self.add_vx_to_i(op),
            SET_I_TO_FONT => self.set_i_to_font(op),
            STORE_BCD => self.store_bcd(op),
            STORE_REGISTERS => self.store_registers(op),
            RESTORE_REGISTERS => self.restore_registers(op),
            _ => unreachable!(
                "F### instruction: invalid suffix: instruction is 0x{:04X} (pc = 0x{:04X}",
                op, self.pc
            ),
        }
    }

    /// op: `FX07`
    /// Store the current value of the delay timer in register `VX`
    fn store_delay(&mut self, op: u16) {
        let vx = (op & 0x0F00) >> 8;
        self.registers[vx as usize] = self.register_delay;
        self.pc += OP_LENGTH;
    }

    /// op: `FX0A`
    /// Wait for a keypress and store the result in register `VX`
    fn wait_for_key(&mut self, op: u16) {
        let vx = (op & 0x0F00) >> 8;
        println!("Waiting for key...");
        self.waiting_for_key = true;
        self.waiting_for_key_vx = vx as u8;
        self.pc += OP_LENGTH;
    }

    /// op: `FX15`
    /// Set the delay timer to the value of register `VX`
    fn set_delay(&mut self, op: u16) {
        let vx = (op & 0x0F00) >> 8;
        self.register_delay = self.registers[vx as usize];
        self.pc += OP_LENGTH;
    }

    /// op: `FX15`
    /// Set the sound timer to the value of register `VX`
    fn set_sound(&mut self, op: u16) {
        let vx = (op & 0x0F00) >> 8;
        self.register_sound = self.registers[vx as usize];
        self.pc += OP_LENGTH;
    }

    /// op: `FX1E`
    /// Add the value stored in register `VX` to register `I`
    fn add_vx_to_i(&mut self, op: u16) {
        let vx = (op & 0x0F00) >> 8;
        self.register_i += self.registers[vx as usize] as u16;
        self.pc += OP_LENGTH;
    }

    /// op: `FX29`
    /// Set `I` to the memory address of the sprite data corresponding to the hexadecimal digit stored in register `VX`
    fn set_i_to_font(&mut self, op: u16) {
        let vx = ((op & 0x0F00) >> 8) as usize;
        self.register_i = FONT_SPRITES_ADDR + self.registers[vx] as u16 * FONT_SPRITE_SIZE;
        self.pc += OP_LENGTH;
    }

    /// op: `FX33`
    /// Store the binary-coded decimal equivalent of the value stored in register VX at addresses `I`, `I + 1`, and `I + 2`
    fn store_bcd(&mut self, op: u16) {
        let vx = (op & 0x0F00) >> 8;
        todo!("Implement BCD");
        self.pc += OP_LENGTH;
    }

    /// op: `FX55`
    /// Store the values of registers `V0` to `VX` inclusive in memory starting at address `I`<br>
    /// `I` is set to `I + X + 1` after operation
    fn store_registers(&mut self, op: u16) {
        let vx = (op & 0x0F00) >> 8;
        for vi in 0..=vx {
            let ram_pos = (self.register_i + vi) as usize;
            self.ram[ram_pos] = self.registers[vi as usize];
        }
        self.pc += OP_LENGTH;
    }

    /// op: `FX65`
    /// Fill registers `V0` to `VX` inclusive with the values stored in memory starting at address `I`<br>
    /// `I` is set to `I + X + 1` after operation
    fn restore_registers(&mut self, op: u16) {
        let vx = (op & 0x0F00) >> 8;
        for vi in 0..=vx {
            let ram_pos = (self.register_i + vi) as usize;
            self.registers[vi as usize] = self.ram[ram_pos];
        }
        self.pc += OP_LENGTH;
    }
}

pub enum KeyState {
    /// The key has been pressed during the last frame
    Pressed,

    /// The key has been released during the last frame
    Released,
}
