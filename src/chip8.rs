extern crate rand;
extern crate find_folder;
extern crate piston;
extern crate piston_window;
extern crate graphics;
extern crate opengl_graphics;
extern crate rodio;

use std::time::{Duration, SystemTime};
use opengl_graphics::{ GlGraphics, OpenGL };
use piston_window::*;
use rand::Rng;

pub struct Chip8 {
    memory: [u8; 0x1000],
    v: [u8; 16],
    i: u16,
    delay: u8,
    sound: u8,
    pc: u16,
    sp: u16,
    stack: [u16; 16],
    halt: bool,
    display: [[u8; 64]; 32],
    next_clock: SystemTime,
    next_timer: SystemTime,
    clock_duration: Duration,
    timer_duration: Duration,
    pause: bool,
    keys: [bool; 16],
    is_waiting: bool,
    waiting_register: usize,
}

impl Chip8 {
    const DEBUG_MODE: bool = false;
    const FONT: [u8; 80] = [
        // 0
        0b11110000,
        0b10010000,
        0b10010000,
        0b10010000,
        0b11110000,
        // 1
        0b00100000,
        0b01100000,
        0b00100000,
        0b00100000,
        0b01110000,
        // 2
        0b11110000,
        0b00010000,
        0b11110000,
        0b10000000,
        0b11110000,
        // 3
        0b11110000,
        0b00010000,
        0b11110000,
        0b00010000,
        0b11110000,
        // 4
        0b10010000,
        0b10010000,
        0b11110000,
        0b00010000,
        0b00010000,
        // 5
        0b11110000,
        0b10000000,
        0b11110000,
        0b00010000,
        0b11110000,
        // 6
        0b11110000,
        0b10000000,
        0b11110000,
        0b10010000,
        0b11110000,
        // 7
        0b11110000,
        0b00010000,
        0b00100000,
        0b01000000,
        0b01000000,
        // 8
        0b11110000,
        0b10010000,
        0b11110000,
        0b10010000,
        0b11110000,
        // 9
        0b11110000,
        0b10010000,
        0b11110000,
        0b00010000,
        0b11110000,
        // A
        0b11110000,
        0b10010000,
        0b11110000,
        0b10010000,
        0b10010000,
        // B
        0b11100000,
        0b10010000,
        0b11100000,
        0b10010000,
        0b11100000,
        // C
        0b11110000,
        0b10000000,
        0b10000000,
        0b10000000,
        0b11110000,
        // D
        0b11100000,
        0b10010000,
        0b10010000,
        0b10010000,
        0b11100000,
        // E
        0b11110000,
        0b10000000,
        0b11110000,
        0b10000000,
        0b11110000,
        // F
        0b11110000,
        0b10000000,
        0b11110000,
        0b10000000,
        0b10000000
    ];
    
    pub fn new(buffer: &Vec<u8>) -> Chip8 {
        let mut memory = [0; 0x1000];
        for i in 0..buffer.len() {
            memory[i+0x200] = buffer[i];
        }
        let mut new_chip8 = Chip8 {
            memory,
            v: [0; 16], i: 0,
            delay: 0, sound: 0,
            pc: 0x200,
            sp: 0, stack: [0; 16],
            halt: false,
            display: [[0; 64]; 32],
            next_clock: SystemTime::now(),
            next_timer: SystemTime::now(),
            clock_duration: Duration::new(0, 181852), // ~540 Hz
            timer_duration: Duration::new(0, 16666667), // ~60 Hz
            pause: false,
            keys: [false; 16],
            is_waiting: false,
            waiting_register: 0,
        };
        new_chip8.init_font();
        new_chip8
    }

    pub fn clock(&mut self) {
        let u_ptr = usize::from(self.pc);
        let cur_instruction = u16::from(self.memory[u_ptr]) << 8 | u16::from(self.memory[u_ptr+1]);
        if cur_instruction == 0x0000 {
            self.halt = true;
            return;
        }
        let hbit = cur_instruction >> 12;
        match hbit {
            0x0 => {
                if cur_instruction == 0x00e0 {
                    self.clear(cur_instruction);
                } else if cur_instruction == 0x00ee {
                    self.return_subroutine(cur_instruction);
                } else {
                    self.call(cur_instruction);
                }
            },
            0x1 => self.jump(cur_instruction),
            0x2 => self.call_subroutine(cur_instruction),
            0x3 => self.skip_eq_xkk(cur_instruction),
            0x4 => self.skip_ne_xkk(cur_instruction),
            0x5 if cur_instruction & 0x000f == 0 => self.skip_eq_xy(cur_instruction),
            0x6 => self.set_vx_kk(cur_instruction),
            0x7 => self.add_vx_kk(cur_instruction),
            0x8 => {
                match cur_instruction & 0x000f {
                    0x0 => self.set_vx_vy(cur_instruction),
                    0x1 => self.or_vx_vy(cur_instruction),
                    0x2 => self.and_vx_vy(cur_instruction),
                    0x3 => self.xor_vx_vy(cur_instruction),
                    0x4 => self.add_vx_vy(cur_instruction),
                    0x5 => self.sub_vx_vy(cur_instruction),
                    0x6 => self.shr_vx_vy(cur_instruction),
                    0x7 => self.subn_vx_vy(cur_instruction),
                    0xE => self.shl_vx_vy(cur_instruction),
                    _ => panic!("OPERATION NOT SUPPORTED!")
                }
            },
            0x9 => self.skip_ne_xy(cur_instruction),
            0xA => self.set_i_nnn(cur_instruction),
            0xB => self.jump_v0(cur_instruction),
            0xC => self.rnd(cur_instruction),
            0xD => self.draw(cur_instruction),
            0xE => {
                if cur_instruction & 0x00ff == 0x9E {
                    self.skip_key_pressed(cur_instruction);
                } else if cur_instruction & 0x00ff == 0xA1 {
                    self.skip_key_not_pressed(cur_instruction);
                } else {
                    panic!("OPERATION NOT SUPPORTED!");
                }
            },
            0xF => {
                match cur_instruction & 0x00ff {
                    0x07 => self.set_vx_dt(cur_instruction),
                    0x0A => self.wait_key(cur_instruction),
                    0x15 => self.set_dt_vx(cur_instruction),
                    0x18 => self.set_sound_vx(cur_instruction),
                    0x1E => self.add_i_vx(cur_instruction),
                    0x29 => self.load_sprite(cur_instruction),
                    0x33 => self.bcd(cur_instruction),
                    0x55 => self.load_v0_vx_i(cur_instruction),
                    0x65 => self.load_i_v0_vx(cur_instruction),
                    _ => panic!("OPERATION NOT SUPPORTED!"),
                }
            },
            _ => panic!("OPERATION NOT SUPPORTED!")
        }
    }

    pub fn run(&mut self) {
        let opengl = OpenGL::V3_2;
        let mut window: PistonWindow =
            WindowSettings::new("CHIP8", [640, 320]).graphics_api(opengl)
            .exit_on_esc(true).build().unwrap();
        window.set_ups(1000);
        let ref mut gl = GlGraphics::new(opengl);
        
        let assets = find_folder::Search::ParentsThenKids(3, 3).for_folder("assets").unwrap();
        println!("{:?}", assets);
        let device = rodio::default_output_device().unwrap();
        let sink = rodio::Sink::new(&device);
        let source = rodio::source::SineWave::new(440);
        sink.pause();
        sink.append(source);

        while let Some(event) = window.next() {
            if self.halt {
                break;
            }
            let mut button: Option<piston_window::Button> = None;
            let mut button_pressed: bool = false;

            if let Some(cur_button) = event.press_args() {
                button = Some(cur_button);
                button_pressed = true;
            }
            if let Some(cur_button) = event.release_args() {
                button = Some(cur_button);
            }

            if button != None {
                use piston_window::Button::Keyboard;
                let key = match button.unwrap() {
                    Keyboard(Key::D1) => 0x1,   //  Real keys |  Chip8
                    Keyboard(Key::D2) => 0x2,   //  1 2 3 4   |  1 2 3 C
                    Keyboard(Key::D3) => 0x3,   //  Q W E R   |  4 5 6 D
                    Keyboard(Key::D4) => 0xC,   //  A S D F   |  7 8 9 E
                    Keyboard(Key::Q) => 0x4,    //  Z X C V   |  A 0 B F
                    Keyboard(Key::W) => 0x5,
                    Keyboard(Key::E) => 0x6,
                    Keyboard(Key::R) => 0xD,
                    Keyboard(Key::A) => 0x7,
                    Keyboard(Key::S) => 0x8,
                    Keyboard(Key::D) => 0x9,
                    Keyboard(Key::F) => 0xE,
                    Keyboard(Key::Z) => 0xA,
                    Keyboard(Key::X) => 0x0,
                    Keyboard(Key::C) => 0xB,
                    Keyboard(Key::V) => 0xF,
                    _ => 0x10,
                };
                if key < 0x10 {
                    self.keys[key as usize] = button_pressed;
                    if button_pressed && self.is_waiting {
                        self.v[self.waiting_register] = key;
                        self.is_waiting = false;
                        self.next_clock = SystemTime::now();
                    }
                } else if button == Some(Keyboard(Key::Space)) && button_pressed {
                    self.toggle_pause();
                } else if button == Some(Keyboard(Key::P)) && !self.is_waiting && button_pressed {
                    self.clock();
                }
            }

            if !self.pause && !self.is_waiting && self.next_clock <= SystemTime::now() {
                self.next_clock += self.clock_duration;
                self.clock();
            }

            if !self.pause && self.next_timer <= SystemTime::now() {
                self.next_timer += self.timer_duration;
                if self.delay > 0 {
                    self.delay -= 1;
                }
                if self.sound > 0 {
                    self.sound -= 1;
                }
            }
            if self.sound > 0 && sink.is_paused() {
                sink.play();
            }
            if self.sound == 0 && !sink.is_paused() {
                sink.pause();
            }

            if let Some(args) = event.render_args() {
                // if next_frame <= SystemTime::now() {
                    // next_frame += frame_timer;
                    gl.draw(args.viewport(), |context, graphics| {
                        graphics::clear([0.0, 0.0, 0.0, 1.0], graphics);
                    
                        // Displaying the screen
                        for i in 0..32 {
                            for j in 0..64 {
                                if self.display[i][j] != 0 {
                                    graphics::rectangle(
                                        [1.0, 1.0, 1.0, 1.0],
                                        [j as f64 * 10.0, i as f64 * 10.0, 10.0, 10.0],
                                        context.transform,
                                        graphics,
                                    )
                                }
                            }
                        }
                    });
                // }
            }
        }
    }

    fn init_font(&mut self) {
        for i in 0..Chip8::FONT.len() {
            self.memory[i] = Chip8::FONT[i];
        }
    }

    fn pause(&mut self) {
        self.pause = true;
    }

    fn unpause(&mut self) {
        self.pause = false;
        self.next_clock = SystemTime::now();
        self.next_timer = SystemTime::now();
    }

    pub fn toggle_pause(&mut self) {
        if self.pause {
            self.unpause();
        } else {
            self.pause();
        }
    }
}

impl Chip8 {
    fn call(&mut self, instruction: u16) {
        if Chip8::DEBUG_MODE {
            println!("{:04x} {:04x}: CALL", self.pc, instruction);
        }
        self.pc += 2;
    }

    fn clear(&mut self, instruction: u16) {
        self.display = [[0; 64]; 32];
        if Chip8::DEBUG_MODE {
            println!("{:04x} {:04x}: CLEAR_SCR", self.pc, instruction);
        }
        self.pc += 2;
    }

    fn return_subroutine(&mut self, instruction: u16) {
        if Chip8::DEBUG_MODE {
            println!("{:04x} {:04x}: RETURN({:04x})", self.pc, instruction, self.stack[usize::from(self.sp-1)]);
        }

        self.pc = self.stack[usize::from(self.sp-1)] + 2;
        self.sp -= 1;
    }

    fn jump(&mut self, instruction: u16) {
        let nnn = instruction & 0x0fff;
        if Chip8::DEBUG_MODE {
            println!("{:04x} {:04x}: JUMP({:04x})", self.pc, instruction, nnn);
        }
        self.pc = nnn;
    }

    fn call_subroutine(&mut self, instruction: u16) {
        let nnn = instruction & 0x0fff;
        if Chip8::DEBUG_MODE {
            println!("{:04x} {:04x}: CALL_SUB({:04x})", self.pc, instruction, nnn);
        }
        self.stack[usize::from(self.sp)] = self.pc;
        self.sp += 1;
        self.pc = nnn;
    }

    fn skip_eq_xkk(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        let kk = (instruction & 0x00ff) as u8;
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: SKIP IF(V[{:02x}]({:02x})=={:02x}) -> {}",
                self.pc,
                instruction,
                x,
                self.v[x],
                kk,
                self.v[x] == kk
            );
        }
        if self.v[x] == kk {
            self.pc += 2;
        }
        self.pc += 2;
    }

    fn skip_ne_xkk(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        let kk = (instruction & 0x00ff) as u8;
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: SKIP IF(V[{:02x}]({:02x})!={:02x}) -> {}",
                self.pc,
                instruction,
                x,
                self.v[x],
                kk,
                self.v[x] != kk
            );
        }
        if self.v[x] != kk {
            self.pc += 2;
        }
        self.pc += 2;
    }

    fn skip_eq_xy(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        let y = usize::from((instruction & 0x00f0) >> 4);
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: SKIP IF(V[{:02x}]({:02x})==V[{:02x}]({:02x})) -> {}",
                self.pc,
                instruction,
                x,
                self.v[x],
                y,
                self.v[y],
                self.v[x] == self.v[y]
            );
        }
        if self.v[x] == self.v[y] {
            self.pc += 2;
        }
        self.pc += 2;
    }

    fn set_vx_kk(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        let kk = (instruction & 0x00ff) as u8;
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: SET V[{:02x}]({:02x})={:02x}",
                self.pc,
                instruction,
                x,
                self.v[x],
                kk
            );
        }
        self.v[x] = kk;
        self.pc += 2;
    }

    fn add_vx_kk(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        let kk = (instruction & 0x00ff) as u8;
        let (res, _overflow) = self.v[x].overflowing_add(kk);
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: ADD V[{:02x}]({:02x})+={:02x} -> {:02x}",
                self.pc,
                instruction,
                x,
                self.v[x],
                kk,
                res
            );
        }
        self.v[x] = res;
        self.pc += 2;
    }

    fn set_vx_vy(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        let y = usize::from((instruction & 0x00f0) >> 4);
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: SET V[{:02x}]({:02x})=V[{:02x}]({:02x})",
                self.pc,
                instruction,
                x,
                self.v[x],
                y,
                self.v[y],
            );
        }
        self.v[x] = self.v[y];
        self.pc += 2;
    }

    fn or_vx_vy(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        let y = usize::from((instruction & 0x00f0) >> 4);
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: OR V[{:02x}]({:02x})|=V[{:02x}]({:02x}) -> {:02x}",
                self.pc,
                instruction,
                x,
                self.v[x],
                y,
                self.v[y],
                self.v[x] | self.v[y],
            );
        }
        self.v[x] |= self.v[y];
        self.pc += 2;
    }

    fn and_vx_vy(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        let y = usize::from((instruction & 0x00f0) >> 4);
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: AND V[{:02x}]({:02x})&=V[{:02x}]({:02x}) -> {:02x}",
                self.pc,
                instruction,
                x,
                self.v[x],
                y,
                self.v[y],
                self.v[x] & self.v[y],
            );
        }
        self.v[x] &= self.v[y];
        self.pc += 2;
    }

    fn xor_vx_vy(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        let y = usize::from((instruction & 0x00f0) >> 4);
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: XOR V[{:02x}]({:02x})^=V[{:02x}]({:02x}) -> {:02x}",
                self.pc,
                instruction,
                x,
                self.v[x],
                y,
                self.v[y],
                self.v[x] ^ self.v[y],
            );
        }
        self.v[x] ^= self.v[y];
        self.pc += 2;
    }

    fn add_vx_vy(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        let y = usize::from((instruction & 0x00f0) >> 4);
        let (res, carry) = self.v[x].overflowing_add(self.v[y]);
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: ADD V[{:02x}]({:02x})+=V[{:02x}]({:02x}) -> {:02x}",
                self.pc,
                instruction,
                x,
                self.v[x],
                y,
                self.v[y],
                res,
            );
        }
        self.v[0xf] = (!carry) as u8;
        self.v[x] = res;
        self.pc += 2;
    }

    fn sub_vx_vy(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        let y = usize::from((instruction & 0x00f0) >> 4);
        let (res, carry) = self.v[x].overflowing_sub(self.v[y]);
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: SUB V[{:02x}]({:02x})-=V[{:02x}]({:02x}) -> {:02x}",
                self.pc,
                instruction,
                x,
                self.v[x],
                y,
                self.v[y],
                res,
            );
        }
        
        self.v[x] = res;
        self.v[0xf] = (!carry) as u8;
        self.pc += 2;
    }

    fn shr_vx_vy(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        // let y = usize::from((instruction & 0x00f0) >> 4);
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: SHR V[{:02x}]({:02x}) -> {:02x}",
                self.pc,
                instruction,
                x,
                self.v[x],
                self.v[x] >> 1,
            );
        }
        self.v[0xf] = self.v[x] & 1;
        self.v[x] >>= 1;
        self.pc += 2;
    }

    fn subn_vx_vy(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        let y = usize::from((instruction & 0x00f0) >> 4);
        let (res, carry) = self.v[y].overflowing_sub(self.v[x]);
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: SUBN V[{:02x}]({:02x})-=V[{:02x}]({:02x}) -> {:02x}",
                self.pc,
                instruction,
                x,
                self.v[x],
                y,
                self.v[y],
                res,
            );
        }
        self.v[0xf] = (!carry) as u8;
        self.v[x] = res;
        self.pc += 2;
    }

    fn shl_vx_vy(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        // let y = usize::from((instruction & 0x00f0) >> 4);
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: SHL V[{:02x}]({:02x}) -> {:02x}",
                self.pc,
                instruction,
                x,
                self.v[x],
                self.v[x] << 1,
            );
        }
        self.v[0xf] = self.v[x] & (1 << 7);
        self.v[x] <<= 1;
        self.pc += 2;
    }

    fn skip_ne_xy(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        let y = usize::from((instruction & 0x00f0) >> 4);
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: SKIP IF(V[{:02x}]({:02x})!=V[{:02x}]({:02x})) -> {}",
                self.pc,
                instruction,
                x,
                self.v[x],
                y,
                self.v[y],
                self.v[x] != self.v[y]
            );
        }
        if self.v[x] != self.v[y] {
            self.pc += 2;
        }
        self.pc += 2;
    }

    fn set_i_nnn(&mut self, instruction: u16) {
        let nnn = instruction & 0x0fff;
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: SET I({:02x})={:04x}",
                self.pc,
                instruction,
                self.i,
                nnn,
            );
        }
        self.i = nnn;
        self.pc += 2;
    }

    fn jump_v0(&mut self, instruction: u16) {
        let nnn = instruction & 0x0fff;
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: JP V[0]({:02x})+{:04x} -> {:04x}",
                self.pc,
                instruction,
                self.v[0],
                nnn,
                u16::from(self.v[0]) + nnn,
            );
        }
        self.pc = u16::from(self.v[0]) + nnn;
    }

    fn rnd(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        let kk = (instruction & 0x00ff) as u8;
        let random: u8 = rand::thread_rng().gen();
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: RND V[{:02x}]({:02x}) = rnd({:02x}) AND {:02x} -> {:02x}",
                self.pc,
                instruction,
                x,
                self.v[x],
                random,
                kk,
                random & kk,
            );
        }
        self.v[x] = random & kk;
        self.pc += 2;
    }
    
    fn draw(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        let y = usize::from((instruction & 0x00f0) >> 4);
        let n = (instruction & 0x000f) as u8;
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: DRW V[{:02x}]({:02x}) V[{:02x}]({:02x}) N({:02x})",
                self.pc,
                instruction,
                x,
                self.v[x],
                y,
                self.v[y],
                n,
            );
        }
        self.v[0xf] = 0;
        for i in 0..n {
            for j in 0..8 {
                let nx = (self.v[y] as u16 + i as u16) as usize & 0b11111;
                let ny = (self.v[x] as u16 + j as u16) as usize & 0b111111;
                let bit = self.memory[self.i as usize + i as usize] & (1 << (7-j));

                if bit != 0 {
                    self.display[nx][ny] ^= 1;
                    if self.display[nx][ny] == 0 {
                        self.v[0xf] = 1;
                    }
                }
            }
        }
        self.pc += 2;
    }

    fn skip_key_pressed(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        if Chip8::DEBUG_MODE {
            println!("{:04x} {:04x}: SKIP KP V[{:02x}]({:02x}) -> {}", self.pc, instruction, x, self.v[x], self.keys[self.v[x] as usize]);
        }
        if self.keys[self.v[x] as usize] {
            self.pc += 2;
        }
        self.pc += 2;
    }

    fn skip_key_not_pressed(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        if Chip8::DEBUG_MODE {
            println!("{:04x} {:04x}: SKIP NKP V[{:02x}]({:02x}) -> {}", self.pc, instruction, x, self.v[x], !self.keys[self.v[x] as usize]);
        }
        if !self.keys[self.v[x] as usize] {
            self.pc += 2;
        }
        self.pc += 2;
    }

    fn set_vx_dt(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        if Chip8::DEBUG_MODE {
            println!("{:04x} {:04x}: LD V[{:02x}]({:02x}) = DT({:02x})", self.pc, instruction, x, self.v[x], self.delay);
        }
        self.v[x] = self.delay;
        self.pc += 2;
    }

    // Fx0A - LD Vx, K
    // Wait for a key press, store the value of the key in Vx.
    // All execution stops until a key is pressed, then the value of that key is stored in Vx.
    fn wait_key(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        if Chip8::DEBUG_MODE {
            println!("{:04x} {:04x}: WAIT V[{:02x}]", self.pc, instruction, x);
        }
        self.waiting_register = x;
        self.is_waiting = true;
        self.pc += 2;
    }

    // Fx15 - LD DT, Vx
    // Set delay timer = Vx.
    // DT is set equal to the value of Vx.
    fn set_dt_vx(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        if Chip8::DEBUG_MODE {
            println!("{:04x} {:04x}: LD DT({:02x}) = V[{:02x}]({:02x})", self.pc, instruction, self.delay, x, self.v[x]);
        }
        self.delay = self.v[x];
        self.pc += 2;
    }

    // Fx18 - LD ST, Vx
    // Set sound timer = Vx.
    // ST is set equal to the value of Vx.
    fn set_sound_vx(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        if Chip8::DEBUG_MODE {
            println!("{:04x} {:04x}: LD ST({:02x}) = V[{:02x}]({:02x})", self.pc, instruction, self.sound, x, self.v[x]);
        }
        self.sound = self.v[x];
        self.pc += 2;
    }

    // Fx1E - ADD I, Vx
    // Set I = I + Vx.
    // The values of I and Vx are added, and the results are stored in I.
    fn add_i_vx(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: I({:04x}) += V[{:02x}]({:02x}) -> {:04x}",
                self.pc,
                instruction,
                self.i,
                x,
                self.v[x],
                self.i + (self.v[x] as u16)
            );
        }

        self.i += self.v[x] as u16;
        self.pc += 2;
    }

    // Fx29 - LD F, Vx
    // Set I = location of sprite for digit Vx.
    // The value of I is set to the location for the hexadecimal sprite corresponding to the value of Vx.
    fn load_sprite(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: LD I({:04x}) = SPRITE(V[{:02x}]({:02x}))",
                self.pc,
                instruction,
                self.i,
                x,
                self.v[x]
            );
        }
        self.i = u16::from(self.v[x]) * 5;
        self.pc += 2;
    }

    // Fx33 - LD B, Vx
    // Store BCD representation of Vx in memory locations I, I+1, and I+2.
    // The interpreter takes the decimal value of Vx, and places the hundreds digit in memory at location in I, the tens digit at location I+1, and the ones digit at location I+2.
    fn bcd(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        if Chip8::DEBUG_MODE {
            println!("{:04x} {:04x}: LD BCD V[{:02x}]({:02x})", self.pc, instruction, x, self.v[x]);
        }
        let mut num = self.v[x];
        self.memory[usize::from(self.i+2)] = num % 10;
        num /= 10;
        self.memory[usize::from(self.i+1)] = num % 10;
        num /= 10;
        self.memory[usize::from(self.i)] = num % 10;
        self.pc += 2;
    }

    // Fx55 - LD [I], Vx
    // Store registers V0 through Vx in memory starting at location I.
    // The interpreter copies the values of registers V0 through Vx into memory, starting at the address in I.
    fn load_v0_vx_i(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: LD MEM[I({:04x})..(I+{:02x})({:04x})] = V[0..{:02x}]",
                self.pc,
                instruction,
                self.i,
                x,
                self.i+(x as u16),
                x,
            );
        }
        for i in 0..x+1 {
            self.memory[usize::from(self.i)+i] = self.v[i];
        }
        self.pc += 2;
    }

    // Fx65 - LD Vx, [I]
    // Read registers V0 through Vx from memory starting at location I.
    // The interpreter reads values from memory starting at location I into registers V0 through Vx.
    fn load_i_v0_vx(&mut self, instruction: u16) {
        let x = usize::from((instruction & 0x0f00) >> 8);
        if Chip8::DEBUG_MODE {
            println!(
                "{:04x} {:04x}: LD V[0..{:02x}] = MEM[I({:04x})..(I+{:02x})({:04x})]",
                self.pc,
                instruction,
                x,
                self.i,
                x,
                self.i+(x as u16),
            );
        }
        for i in 0..x+1 {
            self.v[i] = self.memory[usize::from(self.i)+i];
        }
        self.pc += 2;
    }
}