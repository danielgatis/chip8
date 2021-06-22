use rand;
use rand::Rng;
use std::{thread, time};
use std::env;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{stdout, Read, Write};
use std::process;
use termion::async_stdin;
use termion::raw::IntoRawMode;

fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: chip8 <rom>");
        process::exit(1);
    }

    let mut f = File::open(&args[1]).unwrap();
    let mut rom = [0u8; 3584];
    f.read(&mut rom).unwrap();

    let freq = time::Duration::from_secs_f32(1.0 / 480.0);

    let mut vram = [[0u8; 64]; 32];
    let mut ram = [0u8; 4096];
    let mut stack = [0u16; 16];

    let mut rv = [0u8; 16];
    let mut ri = 0u16;
    let mut pc = 512u16;
    let mut sp = 0u16;

    let mut delay_timer = 0u8;
    let mut sound_timer = 0u8;
    let mut frame_ready = false;

    let mut keypad_x = 0u8;
    let mut keypad_waiting = false;
    let mut keypad_delay = [time::Instant::now().checked_sub(time::Duration::from_secs(5)).unwrap(); 16];
    let mut keypad = [false; 16];

    let fonts: [u8; 80] = [
        0xF0, 0x90, 0x90, 0x90, 0xF0, 0x20, 0x60, 0x20, 0x20, 0x70, 0xF0, 0x10, 0xF0, 0x80,
        0xF0, 0xF0, 0x10, 0xF0, 0x10, 0xF0, 0x90, 0x90, 0xF0, 0x10, 0x10, 0xF0, 0x80, 0xF0,
        0x10, 0xF0, 0xF0, 0x80, 0xF0, 0x90, 0xF0, 0xF0, 0x10, 0x20, 0x40, 0x40, 0xF0, 0x90,
        0xF0, 0x90, 0xF0, 0xF0, 0x90, 0xF0, 0x10, 0xF0, 0xF0, 0x90, 0xF0, 0x90, 0x90, 0xE0,
        0x90, 0xE0, 0x90, 0xE0, 0xF0, 0x80, 0x80, 0x80, 0xF0, 0xE0, 0x90, 0x90, 0x90, 0xE0,
        0xF0, 0x80, 0xF0, 0x80, 0xF0, 0xF0, 0x80, 0xF0, 0x80, 0x80,
    ];

    ram[..fonts.len()].copy_from_slice(&fonts);
    ram[512..].copy_from_slice(&rom);

    let mut stdin = async_stdin().bytes();
    let stdout = stdout();
    let mut stdout = stdout.into_raw_mode().unwrap();
    let mut c = 0;

    write!(stdout, "{}{}", termion::cursor::Hide, termion::clear::All).unwrap();

    loop {
        let start = time::Instant::now();

        let input = match stdin.next() {
            Some(Ok(b'1')) => Some(0x01u8),
            Some(Ok(b'2')) => Some(0x02u8),
            Some(Ok(b'3')) => Some(0x03u8),
            Some(Ok(b'4')) => Some(0x0cu8),
            Some(Ok(b'q')) => Some(0x04u8),
            Some(Ok(b'w')) => Some(0x05u8),
            Some(Ok(b'e')) => Some(0x06u8),
            Some(Ok(b'r')) => Some(0x0du8),
            Some(Ok(b'a')) => Some(0x07u8),
            Some(Ok(b's')) => Some(0x08u8),
            Some(Ok(b'd')) => Some(0x09u8),
            Some(Ok(b'f')) => Some(0x0eu8),
            Some(Ok(b'z')) => Some(0x0au8),
            Some(Ok(b'x')) => Some(0x00u8),
            Some(Ok(b'c')) => Some(0x0bu8),
            Some(Ok(b'v')) => Some(0x0fu8),
            Some(Ok(3)) => break,
            _ => None,
        };

        if let Some(i) = input {
            keypad_delay[i as usize] = time::Instant::now();
        }

        for i in 0..keypad.len() {
            keypad[i] = keypad_delay[i].elapsed().as_millis() <= 100;
        }

        if keypad_waiting {
            for i in 0..keypad.len() {
                if keypad[i] {
                    keypad_waiting = false;
                    rv[keypad_x as usize] = i as u8;
                    break;
                }
            }

            continue;
        }

        let mut rng = rand::thread_rng();

        let opcode = (ram[pc as usize] as u16) << 8 | (ram[(pc + 1) as usize] as u16);
        let o = ((opcode & 0xF000) >> 12) as u8;
        let x = ((opcode & 0x0F00) >> 8) as u8;
        let y = ((opcode & 0x00F0) >> 4) as u8;
        let n = (opcode & 0x000F) as u8;
        let nnn = (opcode & 0x0FFF) as u16;
        let nn = (opcode & 0x00FF) as u8;

        match (o, x, y, n) {
            (0x00, 0x00, 0x0e, 0x00) => {
                vram = [[0u8; 64]; 32];
                frame_ready = true;
                pc += 2;
            }
            (0x00, 0x00, 0x0e, 0x0e) => {
                sp -= 1;
                pc = stack[sp as usize];
            }
            (0x00, _, _, _) => pc += 2,
            (0x01, _, _, _) => pc = nnn,
            (0x02, _, _, _) => {
                stack[sp as usize] = pc + 2;
                sp += 1;
                pc = nnn;
            }
            (0x03, _, _, _) => {
                if rv[x as usize] == nn {
                    pc += 4;
                } else {
                    pc += 2;
                }
            }
            (0x04, _, _, _) => {
                if rv[x as usize] != nn {
                    pc += 4;
                } else {
                    pc += 2;
                }
            }
            (0x05, _, _, 0x00) => {
                if rv[x as usize] == rv[y as usize] {
                    pc += 4;
                } else {
                    pc += 2;
                }
            }
            (0x06, _, _, _) => {
                rv[x as usize] = nn;
                pc += 2;
            }
            (0x07, _, _, _) => {
                rv[x as usize] = (rv[x as usize] as u16 + nn as u16) as u8;
                pc += 2;
            }
            (0x08, _, _, 0x00) => {
                rv[x as usize] = rv[y as usize];
                pc += 2;
            }
            (0x08, _, _, 0x01) => {
                rv[x as usize] |= rv[y as usize];
                pc += 2;
            }
            (0x08, _, _, 0x02) => {
                rv[x as usize] &= rv[y as usize];
                pc += 2;
            }
            (0x08, _, _, 0x03) => {
                rv[x as usize] ^= rv[y as usize];
                pc += 2;
            }
            (0x08, _, _, 0x04) => {
                let result = rv[x as usize] as u16 + rv[y as usize] as u16;
                rv[0x0f] = if result > 0xff { 1 } else { 0 };
                rv[x as usize] = result as u8;
                pc += 2;
            }
            (0x08, _, _, 0x05) => {
                rv[0x0f] = if rv[x as usize] > rv[y as usize] {
                    1
                } else {
                    0
                };
                rv[x as usize] = rv[x as usize].wrapping_sub(rv[y as usize]);
                pc += 2;
            }
            (0x08, _, _, 0x06) => {
                rv[0x0f] = rv[x as usize] & 1;
                rv[x as usize] >>= 1;
                pc += 2;
            }
            (0x08, _, _, 0x07) => {
                rv[0x0f] = if rv[y as usize] > rv[x as usize] {
                    1
                } else {
                    0
                };
                rv[x as usize] = rv[y as usize].wrapping_sub(rv[x as usize]);
                pc += 2;
            }
            (0x08, _, _, 0x0e) => {
                rv[0x0f] = (rv[x as usize] & 0x80) >> 7;
                rv[x as usize] <<= 1;
                pc += 2;
            }
            (0x09, _, _, 0x00) => {
                if rv[x as usize] != rv[y as usize] {
                    pc += 4;
                } else {
                    pc += 2;
                }
            }
            (0x0a, _, _, _) => {
                ri = nnn;
                pc += 2;
            }
            (0x0b, _, _, _) => {
                pc = (rv[0] as u16) + nnn;
            }
            (0x0c, _, _, _) => {
                rv[x as usize] = rng.gen::<u8>() & nn;
                pc += 2;
            }
            (0x0d, _, _, _) => {
                rv[0x0f] = 0;
                for byte in 0..n {
                    let y = (rv[y as usize] + byte) % 32;
                    for bit in 0..8 {
                        let x = (rv[x as usize] + bit) % 64;
                        let color = ram[(ri + byte as u16) as usize] >> (7 - bit) & 1;
                        vram[y as usize][x as usize] ^= color;
                        rv[0x0f] |= color & vram[y as usize][x as usize];
                    }
                }
                frame_ready = true;
                pc += 2;
            }
            (0x0e, _, 0x09, 0x0e) => {
                if keypad[rv[x as usize] as usize] {
                    pc += 4;
                } else {
                    pc += 2;
                }
            }
            (0x0e, _, 0x0a, 0x01) => {
                if !keypad[rv[x as usize] as usize] {
                    pc += 4;
                } else {
                    pc += 2;
                }
            }
            (0x0f, _, 0x00, 0x07) => {
                rv[x as usize] = delay_timer;
                pc += 2;
            }
            (0x0f, _, 0x00, 0x0a) => {
                keypad_waiting = true;
                keypad_x = x;
                pc += 2;
            }
            (0x0f, _, 0x01, 0x05) => {
                delay_timer = rv[x as usize];
                pc += 2;
            }
            (0x0f, _, 0x01, 0x08) => {
                sound_timer = rv[x as usize];
                pc += 2;
            }
            (0x0f, _, 0x01, 0x0e) => {
                ri += rv[x as usize] as u16;
                rv[0x0f] = if ri > 0x0f00 { 1 } else { 0 };
                pc += 2;
            }
            (0x0f, _, 0x02, 0x09) => {
                ri = rv[x as usize] as u16 * 5;
                pc += 2;
            }
            (0x0f, _, 0x03, 0x03) => {
                ram[(ri + 0) as usize] = rv[x as usize] / 100;
                ram[(ri + 1) as usize] = rv[x as usize] % 100 / 10;
                ram[(ri + 2) as usize] = rv[x as usize] % 10;
                pc += 2;
            }
            (0x0f, _, 0x05, 0x05) => {
                for i in 0..x + 1 {
                    ram[(ri + i as u16) as usize] = rv[i as usize];
                }
                pc += 2;
            }
            (0x0f, _, 0x06, 0x05) => {
                for i in 0..x + 1 {
                    rv[i as usize] = ram[(ri + i as u16) as usize];
                }
                pc += 2;
            }
            _ => {
                panic!("invalid opcode: {:#04x}", opcode);
            }
        }

        if delay_timer > 0 {
            delay_timer -= 1;
        }

        if sound_timer > 0 {
            sound_timer -= 1;
        }

        if c % 8 == 0 {
            if sound_timer > 0 {
                write!(stdout, "\x07").unwrap();
            }

            if frame_ready {
                let mut buffer = String::new();

                for y in 0..vram.len() {
                    write!(buffer, "{}", termion::cursor::Goto(1, (y + 1) as u16)).unwrap();

                    for x in 0..vram[y].len() {
                        let fg = if vram[y as usize][x as usize] == 0 {
                            0
                        } else {
                            5
                        };
                        write!(
                            buffer,
                            "{}{}█",
                            termion::color::Fg(termion::color::AnsiValue::rgb(fg, fg, fg)),
                            termion::color::Bg(termion::color::Black)
                        )
                        .unwrap();
                    }
                }

                write!(stdout, "{}{}", buffer, termion::cursor::Goto(1, 33)).unwrap();
                frame_ready = false;
            }
        }

        c += 1;

        let runtime = start.elapsed();
        if let Some(remaining) = freq.checked_sub(runtime) {
            thread::sleep(remaining);
        }
    }
}
