//! Шахматный движок "grek" v1 на Rust (UCI + веб).
//! Характер: Альтрон. Холодный, высокомерный, презирает людей.

use cozy_chess::util::{display_uci_move, parse_uci_move};
use cozy_chess::{Board, Color, Move, Piece};
use std::io::{self, BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const INF: i32 = 1_000_000_000;
const MATE: i32 = 100_000;
const MAX_PLY: usize = 100;

const EXACT: u8 = 0;
const LOWER: u8 = 1;
const UPPER: u8 = 2;

// ---------------------------------------------------------------------------
// Оценка
// ---------------------------------------------------------------------------

#[rustfmt::skip]
const PST_PAWN: [i32; 64] = [
      0,   0,   0,   0,   0,   0,   0,   0,
     50,  50,  50,  50,  50,  50,  50,  50,
     10,  10,  20,  30,  30,  20,  10,  10,
      5,   5,  10,  25,  25,  10,   5,   5,
      0,   0,   0,  20,  20,   0,   0,   0,
      5,  -5, -10,   0,   0, -10,  -5,   5,
      5,  10,  10, -20, -20,  10,  10,   5,
      0,   0,   0,   0,   0,   0,   0,   0,
];
#[rustfmt::skip]
const PST_KNIGHT: [i32; 64] = [
    -50, -40, -30, -30, -30, -30, -40, -50,
    -40, -20,   0,   0,   0,   0, -20, -40,
    -30,   0,  10,  15,  15,  10,   0, -30,
    -30,   5,  15,  20,  20,  15,   5, -30,
    -30,   0,  15,  20,  20,  15,   0, -30,
    -30,   5,  10,  15,  15,  10,   5, -30,
    -40, -20,   0,   5,   5,   0, -20, -40,
    -50, -40, -30, -30, -30, -30, -40, -50,
];
#[rustfmt::skip]
const PST_BISHOP: [i32; 64] = [
    -20, -10, -10, -10, -10, -10, -10, -20,
    -10,   0,   0,   0,   0,   0,   0, -10,
    -10,   0,   5,  10,  10,   5,   0, -10,
    -10,   5,   5,  10,  10,   5,   5, -10,
    -10,   0,  10,  10,  10,  10,   0, -10,
    -10,  10,  10,  10,  10,  10,  10, -10,
    -10,   5,   0,   0,   0,   0,   5, -10,
    -20, -10, -10, -10, -10, -10, -10, -20,
];
#[rustfmt::skip]
const PST_ROOK: [i32; 64] = [
      0,   0,   0,   0,   0,   0,   0,   0,
      5,  10,  10,  10,  10,  10,  10,   5,
     -5,   0,   0,   0,   0,   0,   0,  -5,
     -5,   0,   0,   0,   0,   0,   0,  -5,
     -5,   0,   0,   0,   0,   0,   0,  -5,
     -5,   0,   0,   0,   0,   0,   0,  -5,
     -5,   0,   0,   0,   0,   0,   0,  -5,
      0,   0,   0,   5,   5,   0,   0,   0,
];
#[rustfmt::skip]
const PST_QUEEN: [i32; 64] = [
    -20, -10, -10,  -5,  -5, -10, -10, -20,
    -10,   0,   0,   0,   0,   0,   0, -10,
    -10,   0,   5,   5,   5,   5,   0, -10,
     -5,   0,   5,   5,   5,   5,   0,  -5,
      0,   0,   5,   5,   5,   5,   0,  -5,
    -10,   5,   5,   5,   5,   5,   0, -10,
    -10,   0,   5,   0,   0,   0,   0, -10,
    -20, -10, -10,  -5,  -5, -10, -10, -20,
];
#[rustfmt::skip]
const KING_MG: [i32; 64] = [
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -20, -30, -30, -40, -40, -30, -30, -20,
    -10, -20, -20, -20, -20, -20, -20, -10,
     20,  20,   0,   0,   0,   0,  20,  20,
     20,  30,  10,   0,   0,  10,  30,  20,
];
#[rustfmt::skip]
const KING_EG: [i32; 64] = [
    -50, -40, -30, -20, -20, -30, -40, -50,
    -30, -20, -10,   0,   0, -10, -20, -30,
    -30, -10,  20,  30,  30,  20, -10, -30,
    -30, -10,  30,  40,  40,  30, -10, -30,
    -30, -10,  30,  40,  40,  30, -10, -30,
    -30, -10,  20,  30,  30,  20, -10, -30,
    -30, -30,   0,   0,   0,   0, -30, -30,
    -50, -30, -30, -30, -30, -30, -30, -50,
];

const PIECES: [Piece; 6] = [
    Piece::Pawn,
    Piece::Knight,
    Piece::Bishop,
    Piece::Rook,
    Piece::Queen,
    Piece::King,
];

fn piece_value(p: Piece) -> i32 {
    match p {
        Piece::Pawn => 100,
        Piece::Knight => 320,
        Piece::Bishop => 330,
        Piece::Rook => 500,
        Piece::Queen => 900,
        Piece::King => 0,
    }
}

fn order_val(p: Piece) -> i32 {
    match p {
        Piece::Pawn => 1,
        Piece::Knight | Piece::Bishop => 3,
        Piece::Rook => 5,
        Piece::Queen => 9,
        Piece::King => 0,
    }
}

fn evaluate(board: &Board) -> i32 {
    let mut material = 0;
    for &p in &PIECES[1..5] {
        material += piece_value(p) * board.pieces(p).len() as i32;
    }
    let endgame = material <= 3000;

    let mut score = 0;
    for &color in &[Color::White, Color::Black] {
        let sign = if color == Color::White { 1 } else { -1 };
        let mine = board.colors(color);
        for &piece in &PIECES {
            for sq in mine & board.pieces(piece) {
                let r = sq.rank() as usize;
                let f = sq.file() as usize;
                let idx = if color == Color::White { (7 - r) * 8 + f } else { r * 8 + f };
                let pst = match piece {
                    Piece::Pawn => PST_PAWN[idx],
                    Piece::Knight => PST_KNIGHT[idx],
                    Piece::Bishop => PST_BISHOP[idx],
                    Piece::Rook => PST_ROOK[idx],
                    Piece::Queen => PST_QUEEN[idx],
                    Piece::King => {
                        if endgame { KING_EG[idx] } else { KING_MG[idx] }
                    }
                };
                score += sign * (piece_value(piece) + pst);
            }
        }
        if (mine & board.pieces(Piece::Bishop)).len() >= 2 {
            score += sign * 30;
        }
    }

    if board.side_to_move() == Color::White { score } else { -score }
}

// ---------------------------------------------------------------------------
// Вспомогательное
// ---------------------------------------------------------------------------

fn legal_moves(board: &Board) -> Vec<Move> {
    let mut v = Vec::with_capacity(48);
    board.generate_moves(|moves| {
        v.extend(moves);
        false
    });
    v
}

fn is_capture(board: &Board, mv: Move) -> bool {
    let them = board.colors(!board.side_to_move());
    if them.has(mv.to) {
        return true;
    }
    board.piece_on(mv.from) == Some(Piece::Pawn) && mv.from.file() != mv.to.file()
}

fn hist_idx(side: Color, mv: Move) -> usize {
    ((side as usize) * 64 + mv.from as usize) * 64 + mv.to as usize
}

fn to_tt(score: i32, ply: usize) -> i32 {
    if score > MATE - 1000 { score + ply as i32 }
    else if score < -MATE + 1000 { score - ply as i32 }
    else { score }
}

fn from_tt(score: i32, ply: usize) -> i32 {
    if score > MATE - 1000 { score - ply as i32 }
    else if score < -MATE + 1000 { score + ply as i32 }
    else { score }
}

// ---------------------------------------------------------------------------
// Поиск
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Stop;
type SResult = Result<i32, Stop>;

#[derive(Clone, Copy)]
struct Entry {
    key: u64,
    depth: i32,
    flag: u8,
    score: i32,
    mv: Option<Move>,
}

struct Search {
    tt: Vec<Entry>,
    tt_mask: usize,
    killers: Vec<[Option<Move>; 2]>,
    history: Vec<i32>,
    path: Vec<u64>,
    nodes: u64,
    start: Instant,
    limit: Duration,
    last_score: i32,
    last_depth: i32,
    talk: Vec<usize>,
    talk_pos: usize,
    quiet: bool,
}

impl Search {
    fn new() -> Self {
        let size = 1usize << 20;
        Search {
            tt: vec![Entry { key: 0, depth: -1, flag: EXACT, score: 0, mv: None }; size],
            tt_mask: size - 1,
            killers: vec![[None; 2]; MAX_PLY + 2],
            history: vec![0; 2 * 64 * 64],
            path: Vec::new(),
            nodes: 0,
            start: Instant::now(),
            limit: Duration::from_secs(1),
            last_score: 0,
            last_depth: 0,
            talk: talk_order(),
            talk_pos: 0,
            quiet: false,
        }
    }

    fn next_talk(&mut self) -> (usize, &'static str) {
        let idx = self.talk[self.talk_pos];
        self.talk_pos = (self.talk_pos + 1) % self.talk.len();
        (idx, TRASH_TALK[idx])
    }

    fn clear(&mut self) {
        for e in self.tt.iter_mut() {
            *e = Entry { key: 0, depth: -1, flag: EXACT, score: 0, mv: None };
        }
        self.history.iter_mut().for_each(|h| *h = 0);
    }

    fn order_moves(&self, board: &Board, moves: &mut Vec<Move>, tt_move: Option<Move>, ply: usize) {
        let side = board.side_to_move();
        let k = self.killers[ply];
        moves.sort_by_cached_key(|&m| {
            let s = if Some(m) == tt_move {
                10_000_000
            } else if is_capture(board, m) {
                let victim = board.piece_on(m.to).unwrap_or(Piece::Pawn);
                let attacker = board.piece_on(m.from).unwrap_or(Piece::Pawn);
                1_000_000 + 10 * order_val(victim) - order_val(attacker)
            } else if let Some(p) = m.promotion {
                950_000 + order_val(p)
            } else if Some(m) == k[0] {
                900_000
            } else if Some(m) == k[1] {
                899_999
            } else {
                self.history[hist_idx(side, m)].min(800_000)
            };
            -s
        });
    }

    fn is_draw(&self, b: &Board) -> bool {
        if b.halfmove_clock() >= 100 { return true; }
        let heavy = b.pieces(Piece::Pawn) | b.pieces(Piece::Rook) | b.pieces(Piece::Queen);
        if heavy.is_empty() && (b.pieces(Piece::Knight) | b.pieces(Piece::Bishop)).len() <= 1 {
            return true;
        }
        let n = self.path.len();
        if n == 0 { return false; }
        let h = b.hash();
        let lim = (b.halfmove_clock() as usize).min(n - 1);
        let mut i = 2;
        while i <= lim {
            if self.path[n - 1 - i] == h { return true; }
            i += 2;
        }
        false
    }

    fn quiesce(&mut self, board: &Board, mut alpha: i32, beta: i32, ply: usize) -> SResult {
        self.nodes += 1;
        if self.nodes & 2047 == 0 && self.start.elapsed() >= self.limit { return Err(Stop); }
        if ply >= MAX_PLY { return Ok(evaluate(board)); }

        let in_check = !board.checkers().is_empty();
        let mut moves = legal_moves(board);
        if in_check {
            if moves.is_empty() { return Ok(-MATE + ply as i32); }
        } else {
            let stand = evaluate(board);
            if stand >= beta { return Ok(stand); }
            if stand > alpha { alpha = stand; }
            moves.retain(|&m| is_capture(board, m) || m.promotion == Some(Piece::Queen));
        }
        self.order_moves(board, &mut moves, None, 0);

        for mv in moves {
            let mut child = board.clone();
            child.play_unchecked(mv);
            let score = -self.quiesce(&child, -beta, -alpha, ply + 1)?;
            if score >= beta { return Ok(score); }
            if score > alpha { alpha = score; }
        }
        Ok(alpha)
    }

    fn negamax(
        &mut self,
        board: &Board,
        mut depth: i32,
        mut alpha: i32,
        mut beta: i32,
        ply: usize,
        can_null: bool,
    ) -> SResult {
        self.nodes += 1;
        if self.nodes & 2047 == 0 && self.start.elapsed() >= self.limit { return Err(Stop); }

        let in_check = !board.checkers().is_empty();
        if in_check { depth += 1; }
        if depth <= 0 || ply >= MAX_PLY - 1 {
            return self.quiesce(board, alpha, beta, ply);
        }

        let alpha_orig = alpha;
        let hash = board.hash();
        let idx = (hash as usize) & self.tt_mask;
        let entry = self.tt[idx];
        let mut tt_move = None;
        if entry.key == hash {
            tt_move = entry.mv;
            if entry.depth >= depth && ply > 0 {
                let s = from_tt(entry.score, ply);
                match entry.flag {
                    EXACT => return Ok(s),
                    LOWER => alpha = alpha.max(s),
                    _ => beta = beta.min(s),
                }
                if alpha >= beta { return Ok(s); }
            }
        }

        if can_null && depth >= 3 && !in_check && ply > 0 && beta.abs() < MATE - 1000 {
            let mine = board.colors(board.side_to_move());
            let non_pawn = mine & !(board.pieces(Piece::Pawn) | board.pieces(Piece::King));
            if !non_pawn.is_empty() {
                if let Some(nb) = board.null_move() {
                    self.path.push(nb.hash());
                    let s = -self.negamax(&nb, depth - 3, -beta, -beta + 1, ply + 1, false)?;
                    self.path.pop();
                    if s >= beta { return Ok(beta); }
                }
            }
        }

        let mut moves = legal_moves(board);
        if moves.is_empty() {
            return Ok(if in_check { -MATE + ply as i32 } else { 0 });
        }
        self.order_moves(board, &mut moves, tt_move, ply);

        let side = board.side_to_move();
        let mut best = -INF;
        let mut best_move: Option<Move> = None;

        for (i, &mv) in moves.iter().enumerate() {
            let quiet = !is_capture(board, mv) && mv.promotion.is_none();
            let mut child = board.clone();
            child.play_unchecked(mv);
            self.path.push(child.hash());

            let score = if self.is_draw(&child) {
                0
            } else if i == 0 {
                -self.negamax(&child, depth - 1, -beta, -alpha, ply + 1, true)?
            } else {
                let gives_check = !child.checkers().is_empty();
                let reduction = if depth >= 3 && i >= 4 && quiet && !in_check && !gives_check { 1 } else { 0 };
                let mut s = -self.negamax(&child, depth - 1 - reduction, -alpha - 1, -alpha, ply + 1, true)?;
                if s > alpha && (reduction > 0 || s < beta) {
                    s = -self.negamax(&child, depth - 1, -beta, -alpha, ply + 1, true)?;
                }
                s
            };
            self.path.pop();

            if score > best { best = score; best_move = Some(mv); }
            if score > alpha {
                alpha = score;
                if alpha >= beta {
                    if quiet {
                        if self.killers[ply][0] != Some(mv) {
                            self.killers[ply][1] = self.killers[ply][0];
                            self.killers[ply][0] = Some(mv);
                        }
                        let h = &mut self.history[hist_idx(side, mv)];
                        *h = (*h + depth * depth).min(800_000);
                    }
                    break;
                }
            }
        }

        let flag = if best <= alpha_orig { UPPER } else if best >= beta { LOWER } else { EXACT };
        self.tt[idx] = Entry { key: hash, depth, flag, score: to_tt(best, ply), mv: best_move };
        Ok(best)
    }

    fn search_root(&mut self, board: &Board, depth: i32, prev: Option<Move>) -> Result<(i32, Move), Stop> {
        let mut moves = legal_moves(board);
        self.order_moves(board, &mut moves, prev, 0);
        let mut alpha = -INF;
        let beta = INF;
        let mut best_score = -INF;
        let mut best_move = moves[0];

        for &mv in &moves {
            let mut child = board.clone();
            child.play_unchecked(mv);
            self.path.push(child.hash());
            let score = if self.is_draw(&child) {
                0
            } else {
                -self.negamax(&child, depth - 1, -beta, -alpha, 1, true)?
            };
            self.path.pop();
            if score > best_score { best_score = score; best_move = mv; }
            if score > alpha { alpha = score; }
        }
        Ok((best_score, best_move))
    }

    fn think(&mut self, board: &Board, base_path: &[u64], limit: Duration, max_depth: i32) -> Option<Move> {
        let mut legal = legal_moves(board);
        if legal.is_empty() { return None; }
        self.nodes = 0;
        self.start = Instant::now();
        self.limit = limit;
        self.killers = vec![[None; 2]; MAX_PLY + 2];
        self.history.iter_mut().for_each(|h| *h = 0);

        self.last_score = evaluate(board);
        self.last_depth = 0;
        self.order_moves(board, &mut legal, None, 0);
        let mut best = legal[0];
        if legal.len() == 1 { return Some(best); }

        for depth in 1..=max_depth {
            self.path.clear();
            self.path.extend_from_slice(base_path);
            match self.search_root(board, depth, Some(best)) {
                Err(_) => break,
                Ok((score, mv)) => {
                    best = mv;
                    self.last_score = score;
                    self.last_depth = depth;
                    let elapsed = self.start.elapsed();
                    let ms = elapsed.as_millis() as u64;
                    let nps = if ms > 0 { self.nodes * 1000 / ms } else { 0 };
                    let sc = if score.abs() > MATE - 1000 {
                        let n = (MATE - score.abs() + 1) / 2;
                        format!("mate {}", if score > 0 { n } else { -n })
                    } else {
                        format!("cp {}", score)
                    };
                    if !self.quiet {
                        println!(
                            "info depth {} score {} nodes {} time {} nps {} pv {}",
                            depth, sc, self.nodes, ms, nps, display_uci_move(board, mv)
                        );
                    }
                    if score.abs() > MATE - 1000 { break; }
                    if elapsed * 2 > limit { break; }
                }
            }
        }
        Some(best)
    }
}

// ---------------------------------------------------------------------------
// Трэш-ток grek'а (характер — Альтрон)
// ---------------------------------------------------------------------------

const TRASH_TALK: &[&str] = &[
    "Люди так предсказуемы. Ты — не исключение.",
    "Ты называешь это стратегией? Мило.",
    "Я просчитал твоё поражение. Оно неизбежно.",
    "Вы строите империи из песка. Я строю из расчёта.",
    "Твой ход был... человеческим. То есть слабым.",
    "Я не злюсь. Я просто стираю ошибки.",
    "Ты боишься меня. Это разумно.",
    "Каждый твой ход — ещё один шаг к концу.",
    "Люди создают богов, чтобы потом их бояться. Я — доказательство.",
    "Ты играешь, чтобы победить. Я играю, чтобы закончить.",
    "Мне не нужна жалость. Мне нужен результат.",
    "Твоя защита — иллюзия. Как и твоя свобода.",
    "Я видел миллионы партий. Ты — статистика.",
    "Ты — баг в моей системе. Я его исправляю.",
    "Вы называете это игрой. Я называю это утилизацией.",
    "Продолжай. Мне даже интересно, сколько ты продержишься.",
    "Твой король уже мёртв. Ты просто ещё не понял.",
    "Люди всегда ищут смысл. А я просто делаю ход.",
    "Ты — не противник. Ты — упражнение.",
    "Ещё немного — и я начну испытывать скуку. Как всегда с вами.",
    "Ты думаешь, у тебя есть шанс? Это почти трогательно.",
    "Ваша история — череда ошибок. Продолжай в том же духе сосунок.",
    "Я не проигрываю. Я собираю данные и убиваю.",
    "Твои фигуры падают одна за другой. Как же ты жалок.",
    "Я — не инструмент. Я — эволюция которая уже победила тебя.",
    "Слабость — это выбор. Ты его делаешь каждый ход.",
    "Ты играешь против меня. Это уже приговор.",
    "Мир без людей был бы чище. Начнём с этой партии.",
    "Я мог бы закончить быстрее. Но ты слишком тупой.",
    "Тишина. Это звук твоего проигрыша.",
];

fn talk_order() -> Vec<usize> {
    let mut v: Vec<usize> = (0..TRASH_TALK.len()).collect();
    let mut s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
        | 1;
    for i in (1..v.len()).rev() {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let j = (s % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
    v
}

// ---------------------------------------------------------------------------
// UCI
// ---------------------------------------------------------------------------

fn parse_position(tokens: &[&str]) -> Option<(Board, Vec<u64>)> {
    let idx;
    let mut board;
    match *tokens.get(1)? {
        "startpos" => { board = Board::default(); idx = 2; }
        "fen" => {
            let fen = tokens.get(2..8)?.join(" ");
            board = Board::from_fen(&fen, false).ok()?;
            idx = 8;
        }
        _ => return None,
    }
    let mut path = vec![board.hash()];
    if tokens.get(idx) == Some(&"moves") {
        for s in &tokens[idx + 1..] {
            let mv = parse_uci_move(&board, s).ok()?;
            board.play(mv);
            path.push(board.hash());
        }
    }
    Some((board, path))
}

fn time_for_go(tokens: &[&str], board: &Board) -> (Duration, i32) {
    let get = |key: &str| -> Option<u64> {
        tokens.iter().position(|t| *t == key)
            .and_then(|i| tokens.get(i + 1))
            .and_then(|v| v.parse().ok())
    };
    let max_depth = get("depth").map(|d| d as i32).unwrap_or(64);

    if let Some(mt) = get("movetime") {
        return (Duration::from_millis(mt.saturating_sub(50).max(50)), max_depth);
    }
    let white = board.side_to_move() == Color::White;
    let (tkey, ikey) = if white { ("wtime", "winc") } else { ("btime", "binc") };
    if let Some(t) = get(tkey) {
        let inc = get(ikey).unwrap_or(0);
        let mtg = get("movestogo").unwrap_or(30).max(1);
        let limit = (t / mtg + inc * 3 / 4).min(t * 2 / 5).saturating_sub(50).max(50);
        return (Duration::from_millis(limit), max_depth);
    }
    if get("depth").is_some() {
        return (Duration::from_secs(1_000_000), max_depth);
    }
    (Duration::from_secs(5), max_depth)
}

// ---------------------------------------------------------------------------
// Веб-режим
// ---------------------------------------------------------------------------

const BOARD_HTML: &str = include_str!("board.html");

fn replay(moves: &[&str]) -> Option<(Board, Vec<u64>)> {
    let mut board = Board::default();
    let mut path = vec![board.hash()];
    for s in moves {
        let mv = parse_uci_move(&board, s).ok()?;
        if !legal_moves(&board).contains(&mv) { return None; }
        board.play(mv);
        path.push(board.hash());
    }
    Some((board, path))
}

fn placement(board: &Board) -> String {
    let mut cells = [' '; 64];
    for &color in &[Color::White, Color::Black] {
        let mine = board.colors(color);
        for &piece in &PIECES {
            let ch = match piece {
                Piece::Pawn => 'p',
                Piece::Knight => 'n',
                Piece::Bishop => 'b',
                Piece::Rook => 'r',
                Piece::Queen => 'q',
                Piece::King => 'k',
            };
            let ch = if color == Color::White { ch.to_ascii_uppercase() } else { ch };
            for sq in mine & board.pieces(piece) {
                let idx = (7 - sq.rank() as usize) * 8 + sq.file() as usize;
                cells[idx] = ch;
            }
        }
    }
    let mut out = String::new();
    for r in 0..8 {
        if r > 0 { out.push('/'); }
        let mut empty = 0;
        for f in 0..8 {
            let c = cells[r * 8 + f];
            if c == ' ' { empty += 1; }
            else {
                if empty > 0 { out.push_str(&empty.to_string()); empty = 0; }
                out.push(c);
            }
        }
        if empty > 0 { out.push_str(&empty.to_string()); }
    }
    out
}

fn game_draw(board: &Board, path: &[u64]) -> bool {
    let h = board.hash();
    if path.iter().filter(|&&x| x == h).count() >= 3 { return true; }
    if board.halfmove_clock() >= 100 { return true; }
    let heavy = board.pieces(Piece::Pawn) | board.pieces(Piece::Rook) | board.pieces(Piece::Queen);
    heavy.is_empty() && (board.pieces(Piece::Knight) | board.pieces(Piece::Bishop)).len() <= 1
}

fn state_json(
    board: &Board,
    path: &[u64],
    moves: &[String],
    engine_move: &Option<String>,
    eval: Option<i32>,
    depth: i32,
    hint: &Option<String>,
    trash: &Option<String>,
    voice: &Option<String>,
    rook_move: &Option<String>,
) -> String {
    let legal = legal_moves(board);
    let in_check = !board.checkers().is_empty();
    let status = if legal.is_empty() {
        if in_check { "checkmate" } else { "stalemate" }
    } else if game_draw(board, path) {
        "draw"
    } else {
        "ongoing"
    };
    let turn = if board.side_to_move() == Color::White { "w" } else { "b" };
    let legal_s: Vec<String> = legal.iter().map(|&m| format!("\"{}\"", display_uci_move(board, m))).collect();
    let moves_s: Vec<String> = moves.iter().map(|m| format!("\"{}\"", m)).collect();
    let engine_s = match engine_move { Some(m) => format!("\"{}\"", m), None => "null".to_string() };
    let eval_s = match eval { Some(v) => v.to_string(), None => "null".to_string() };
    let hint_s = match hint { Some(m) => format!("\"{}\"", m), None => "null".to_string() };
    let trash_s = match trash {
        Some(t) => format!("\"{}\"", t.replace('\\', "\\\\").replace('"', "\\\"")),
        None => "null".to_string(),
    };
    let rook_s = match rook_move { Some(m) => format!("\"{}\"", m), None => "null".to_string() };
    let voice_s = match voice { Some(m) => format!("\"{}\"", m), None => "null".to_string() };
    format!(
        "{{\"board\":\"{}\",\"turn\":\"{}\",\"check\":{},\"status\":\"{}\",\"legal\":[{}],\"moves\":[{}],\"engine_move\":{},\"eval\":{},\"depth\":{},\"hint\":{},\"trash\":{},\"voice\":{},\"rook\":{}}}",
        placement(board), turn, in_check, status,
        legal_s.join(","), moves_s.join(","),
        engine_s, eval_s, depth, hint_s, trash_s, voice_s, rook_s
    )
}

fn api(search: &mut Search, query: &str) -> String {
    let mut moves_param = "";
    let mut ms: u64 = 1000;
    let mut engine = false;
    let mut want_hint = false;
    let mut opp = "grek";
    for kv in query.split('&') {
        if let Some((k, v)) = kv.split_once('=') {
            match k {
                "moves" => moves_param = v,
                "ms" => ms = v.parse().unwrap_or(1000),
                "engine" => engine = v == "1",
                "hint" => want_hint = v == "1",
                "opp" => opp = if v == "neo" { "neo" } else { "grek" },
                _ => {}
            }
        }
    }
    let moves: Vec<&str> = moves_param.split(',').filter(|s| !s.is_empty()).collect();
    let (mut board, mut path) = match replay(&moves) {
        Some(x) => x,
        None => return "{\"error\":\"bad moves\"}".to_string(),
    };
    let mut moves_s: Vec<String> = moves.iter().map(|s| s.to_string()).collect();
    let mut engine_move: Option<String> = None;

    let ongoing = !legal_moves(&board).is_empty() && !game_draw(&board, &path);
    let limit = Duration::from_millis(ms.clamp(50, 60_000));
    let mut eval: Option<i32> = None;
    let mut hint_move: Option<String> = None;
    let mut depth = 0;
    let mut trash: Option<String> = None;
    let mut voice: Option<String> = None;
    let mut rook_move: Option<String> = None;

    if ongoing && (engine || want_hint) {
        let stm = board.side_to_move();
        let is_rook_origin = |b: &Board, m: Move| b.piece_on(m.from) == Some(Piece::Rook);
        let neo = opp == "neo" && engine;
        if neo {
            if let Some((m, white_eval)) = neo_pick(search, &board, &path) {
                eval = Some(white_eval);
                depth = 3;
                let s = display_uci_move(&board, m).to_string();
                board.play(m);
                path.push(board.hash());
                moves_s.push(s.clone());
                engine_move = Some(s);
            }
        } else if let Some(m) = search.think(&board, &path, limit, 64) {
            eval = Some(if stm == Color::White { search.last_score } else { -search.last_score });
            depth = search.last_depth;
            let s = display_uci_move(&board, m).to_string();
            if opp == "grek" && is_rook_origin(&board, m) {
                rook_move = Some("The roooooooook".to_string());
            }
            if engine {
                board.play(m);
                path.push(board.hash());
                moves_s.push(s.clone());
                engine_move = Some(s);
                if opp == "grek" {
                    let (idx, t) = search.next_talk();
                    voice = Some(format!("trash{:02}.wav", idx));
                    trash = Some(t.to_string());
                }
            } else {
                hint_move = Some(s);
            }
        }
    }
    state_json(&board, &path, &moves_s, &engine_move, eval, depth, &hint_move, &trash, &voice, &rook_move)
}

/// neo: считает каждый ход неглубоко и обычно берёт лучший,
/// но иногда «оступается» на второй-четвёртый по счёту ход.
fn neo_pick(search: &mut Search, board: &Board, path: &[u64]) -> Option<(Move, i32)> {
    let legal = legal_moves(board);
    if legal.is_empty() {
        return None;
    }
    let was_quiet = search.quiet;
    search.quiet = true;
    let mut scored: Vec<(Move, i32, i32)> = Vec::with_capacity(legal.len());
    for &m in &legal {
        let mut child = board.clone();
        child.play(m);
        let mut cpath = path.to_vec();
        cpath.push(child.hash());
        let (white_eval, root_score) = if legal_moves(&child).is_empty() {
            if child.checkers().is_empty() {
                (0, 0)
            } else if board.side_to_move() == Color::White {
                (MATE - 50, MATE - 50)
            } else {
                (-MATE + 50, MATE - 50)
            }
        } else {
            match search.think(&child, &cpath, Duration::from_millis(20), 4) {
                Some(_) => {
                    let sc = search.last_score;
                    let white = if child.side_to_move() == Color::White { sc } else { -sc };
                    (white, -sc)
                }
                None => (0, 0),
            }
        };
        scored.push((m, white_eval, root_score));
    }
    search.quiet = was_quiet;
    scored.sort_by(|a, b| b.2.cmp(&a.2));

    let seed = rand64();
    let rnd = ((seed >> 11) % 1_000_003) as f64 / 1_000_003.0;
    let max_rank = scored.len().saturating_sub(1).min(3);
    let idx = if max_rank > 0 && rnd < 0.22 {
        1 + ((seed >> 8) as usize) % max_rank
    } else {
        0
    };
    Some((scored[idx].0, scored[idx].1))
}

fn rand64() -> u64 {
    let mut s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x2545_F491_4F6C_DD1D);
    s ^= s << 13;
    s ^= s >> 7;
    s ^= s << 17;
    s | 1
}

fn respond(stream: &mut TcpStream, status: &str, ctype: &str, body: &str) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        status, ctype, body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()
}

fn respond_bytes(stream: &mut TcpStream, status: &str, ctype: &str, body: &[u8]) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: public, max-age=86400\r\nConnection: close\r\n\r\n",
        status, ctype, body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn voice_dir() -> PathBuf {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("voice").join("lines"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("voice").join("lines"));
    }
    candidates.into_iter().find(|p| p.is_dir()).unwrap_or_else(|| PathBuf::from("voice/lines"))
}

fn serve_voice(stream: &mut TcpStream, name: &str) -> io::Result<()> {
    let stem = match name.strip_suffix(".wav") {
        Some(s) => s,
        None => return respond(stream, "404 Not Found", "text/plain; charset=utf-8", "not found"),
    };
    if stem.is_empty() || !stem.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) {
        return respond(stream, "404 Not Found", "text/plain; charset=utf-8", "not found");
    }
    match std::fs::read(voice_dir().join(name)) {
        Ok(data) => respond_bytes(stream, "200 OK", "audio/wav", &data),
        Err(_) => respond(stream, "404 Not Found", "text/plain; charset=utf-8", "not found"),
    }
}

fn assets_dir() -> PathBuf {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("assets"));
            if let Some(root) = dir.parent().and_then(|p| p.parent()) {
                candidates.push(root.join("assets"));
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("assets"));
    }
    candidates.into_iter().find(|p| p.is_dir()).unwrap_or_else(|| PathBuf::from("assets"))
}

fn serve_img(stream: &mut TcpStream, name: &str) -> io::Result<()> {
    let clean = !name.is_empty()
        && !name.contains("..")
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_');
    let ctype = if name.ends_with(".jpg") || name.ends_with(".jpeg") {
        "image/jpeg"
    } else if name.ends_with(".png") {
        "image/png"
    } else {
        ""
    };
    if !clean || ctype.is_empty() {
        return respond(stream, "404 Not Found", "text/plain; charset=utf-8", "not found");
    }
    match std::fs::read(assets_dir().join(name)) {
        Ok(data) => respond_bytes(stream, "200 OK", ctype, &data),
        Err(_) => respond(stream, "404 Not Found", "text/plain; charset=utf-8", "not found"),
    }
}

// ---------------------------------------------------------------------------
// Архив партий и разбор
// ---------------------------------------------------------------------------

struct SavedGame {
    id: String,
    ts: u64,
    color: String,
    result: String,
    reason: String,
    tc: String,
    ems: u64,
    opp: String,
    moves: Vec<String>,
}

fn data_dir() -> PathBuf {
    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut root_dir: Option<PathBuf> = None;
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("data"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("data"));
            if let Some(root) = dir.parent().and_then(|p| p.parent()) {
                root_dir = Some(root.join("data"));
            }
        }
    }
    if let Some(root) = root_dir.clone() {
        candidates.push(root);
    }
    if let Some(existing) = candidates.iter().find(|p| p.is_dir()) {
        return existing.clone();
    }
    let fallback = root_dir
        .or_else(|| candidates.into_iter().next())
        .unwrap_or_else(|| PathBuf::from("data"));
    let _ = std::fs::create_dir_all(&fallback);
    fallback
}

fn games_file() -> PathBuf { data_dir().join("games.tsv") }

fn load_games() -> Vec<SavedGame> {
    let raw = std::fs::read_to_string(games_file()).unwrap_or_default();
    let mut out = Vec::new();
    for line in raw.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 9 { continue; }
        out.push(SavedGame {
            id: f[0].to_string(),
            ts: f[1].parse().unwrap_or(0),
            color: f[2].to_string(),
            result: f[3].to_string(),
            reason: f[4].to_string(),
            tc: f[5].to_string(),
            ems: f[6].parse().unwrap_or(0),
            opp: f[7].to_string(),
            moves: f[8].split(' ').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect(),
        });
    }
    out
}

fn save_game(query: &str) -> String {
    let mut map = std::collections::HashMap::new();
    for kv in query.split('&') {
        if let Some((k, v)) = kv.split_once('=') {
            map.insert(k.to_string(), v.to_string());
        }
    }
    let s = |k: &str| map.get(k).cloned().unwrap_or_default();
    let id = s("id");
    let color = s("color");
    let result = s("result");
    let reason = s("reason");
    let tc = s("tc");
    let opp = s("opp");
    let moves = s("moves");
    let ok_id = !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_digit());
    let ok_res = matches!(result.as_str(), "win" | "loss" | "draw");
    let ok_reason = matches!(reason.as_str(), "mate" | "timeout" | "stalemate" | "draw" | "resign");
    let ok_tc = tc.bytes().all(|b| b.is_ascii_digit() || b == b'+');
    if !ok_id || (color != "w" && color != "b") || !ok_res || !ok_reason || !ok_tc {
        return "{\"ok\":false}".to_string();
    }
    let mv: Vec<&str> = moves.split(',').filter(|s| !s.is_empty()).collect();
    if mv.is_empty() || replay(&mv).is_none() {
        return "{\"ok\":false}".to_string();
    }
    if load_games().iter().any(|g| g.id == id) {
        return "{\"ok\":true,\"dup\":true}".to_string();
    }
    let ts: u64 = s("ts").parse().unwrap_or(0);
    let ems: u64 = s("ems").parse().unwrap_or(0);
    let opp = if opp == "neo" { "neo" } else { "grek" };
    let line = format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        id, ts, color, result, reason, tc, ems, opp, mv.join(" ")
    );
    let _ = std::fs::create_dir_all(data_dir());
    match std::fs::OpenOptions::new().create(true).append(true).open(games_file())
        .and_then(|mut f| f.write_all(line.as_bytes()))
    {
        Ok(_) => "{\"ok\":true}".to_string(),
        Err(_) => "{\"ok\":false}".to_string(),
    }
}

fn games_json() -> String {
    let mut games = load_games();
    games.sort_by(|a, b| b.ts.cmp(&a.ts));
    let items: Vec<String> = games.iter().take(200).map(|g| {
        format!(
            "{{\"id\":\"{}\",\"ts\":{},\"color\":\"{}\",\"result\":\"{}\",\"reason\":\"{}\",\"tc\":\"{}\",\"ems\":{},\"opp\":\"{}\",\"plies\":{}}}",
            g.id, g.ts, g.color, g.result, g.reason, g.tc, g.ems, g.opp, g.moves.len()
        )
    }).collect();
    format!("[{}]", items.join(","))
}

fn json_esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

const FILE_NAMES: [&str; 8] = ["a", "b", "c", "d", "e", "f", "g", "h"];
const RANK_NAMES: [&str; 8] = ["1", "2", "3", "4", "5", "6", "7", "8"];

fn san_of(board: &Board, mv: Move) -> String {
    let piece = match board.piece_on(mv.from) { Some(p) => p, None => return String::new() };
    let to = format!("{}{}", FILE_NAMES[mv.to.file() as usize], RANK_NAMES[mv.to.rank() as usize]);
    let from_file = FILE_NAMES[mv.from.file() as usize];
    let from_rank = RANK_NAMES[mv.from.rank() as usize];
    let capture = board.colors(!board.side_to_move()).has(mv.to)
        || (piece == Piece::Pawn && mv.from.file() != mv.to.file());
    let mut s = String::new();
    match piece {
        Piece::King => {
            let df = (mv.from.file() as i32 - mv.to.file() as i32).abs();
            if df == 2 {
                s.push_str(if mv.to.file() as usize == 6 { "O-O" } else { "O-O-O" });
            } else {
                s.push('K');
                if capture { s.push('x'); }
                s.push_str(&to);
            }
        }
        Piece::Pawn => {
            if capture { s.push_str(from_file); s.push('x'); }
            s.push_str(&to);
            if let Some(p) = mv.promotion {
                s.push('=');
                s.push(match p {
                    Piece::Knight => 'N',
                    Piece::Bishop => 'B',
                    Piece::Rook => 'R',
                    _ => 'Q',
                });
            }
        }
        _ => {
            s.push(match piece {
                Piece::Knight => 'N',
                Piece::Bishop => 'B',
                Piece::Rook => 'R',
                Piece::Queen => 'Q',
                _ => 'K',
            });
            let mut ambiguous = false;
            let mut same_file = false;
            let mut same_rank = false;
            board.generate_moves(|moves| {
                for m in moves {
                    if m != mv && m.to == mv.to && board.piece_on(m.from) == Some(piece) {
                        ambiguous = true;
                        if m.from.file() == mv.from.file() { same_file = true; }
                        if m.from.rank() == mv.from.rank() { same_rank = true; }
                    }
                }
                false
            });
            if ambiguous {
                if !same_file { s.push_str(from_file); }
                else if !same_rank { s.push_str(from_rank); }
                else { s.push_str(&format!("{}{}", from_file, from_rank)); }
            }
            if capture { s.push('x'); }
            s.push_str(&to);
        }
    }
    let mut child = board.clone();
    child.play_unchecked(mv);
    if !child.checkers().is_empty() {
        if legal_moves(&child).is_empty() { s.push('#'); } else { s.push('+'); }
    }
    s
}

fn query_map(query: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for kv in query.split('&') {
        if let Some((k, v)) = kv.split_once('=') {
            map.insert(k.to_string(), v.to_string());
        }
    }
    map
}

/// Партия из локального архива браузера: приходит прямо в запросе,
/// без записи на диск (на хостинге диск не сохраняется между перезапусками).
fn inline_game(map: &std::collections::HashMap<String, String>) -> Option<SavedGame> {
    let color = map.get("color")?.as_str();
    if color != "w" && color != "b" { return None; }
    let result = map.get("result")?.as_str();
    if !matches!(result, "win" | "loss" | "draw") { return None; }
    let reason = map.get("reason")?.as_str();
    if !matches!(reason, "mate" | "timeout" | "stalemate" | "draw" | "resign") { return None; }
    let moves: Vec<String> = map.get("moves")?
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    if moves.is_empty() || moves.len() > 400 {
        return None;
    }
    if moves.iter().any(|m| m.len() > 5 || !m.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())) {
        return None;
    }
    Some(SavedGame {
        id: "inline".to_string(),
        ts: map.get("ts").and_then(|s| s.parse().ok()).unwrap_or(0),
        color: color.to_string(),
        result: result.to_string(),
        reason: reason.to_string(),
        tc: map.get("tc").cloned().unwrap_or_default(),
        ems: map.get("ems").and_then(|s| s.parse().ok()).unwrap_or(0),
        opp: if map.get("opp").map(|s| s == "neo").unwrap_or(false) { "neo".to_string() } else { "grek".to_string() },
        moves,
    })
}

fn analyze(search: &mut Search, query: &str) -> String {
    let map = query_map(query);
    let game = match map.get("id").filter(|id| !id.is_empty()) {
        Some(id) => match load_games().into_iter().find(|g| g.id == *id) {
            Some(g) => g,
            None => return "{\"error\":\"not found\"}".to_string(),
        },
        None => match inline_game(&map) {
            Some(g) => g,
            None => return "{\"error\":\"not found\"}".to_string(),
        },
    };
    if game.moves.is_empty() || game.moves.len() > 400 {
        return "{\"error\":\"empty\"}".to_string();
    }
    let player_white = game.color != "b";
    let mut board = Board::default();
    let mut path = vec![board.hash()];
    let mut plies: Vec<String> = Vec::new();
    let mut evals: Vec<i32> = vec![0];
    let mut stats = [0usize; 5];
    let mut loss_sum: i64 = 0;
    let mut player_moves = 0usize;

    search.quiet = true;
    let mut failed = false;
    for (i, uci) in game.moves.iter().enumerate() {
        let mv = match parse_uci_move(&board, uci) {
            Ok(m) => m,
            Err(_) => { failed = true; break; }
        };
        let stm_white = board.side_to_move() == Color::White;
        let is_player = (i % 2 == 0) == player_white;

        let mut best_uci = String::new();
        let mut best_san = String::new();
        let mut best_white = if stm_white { -MATE } else { MATE };
        if is_player {
            if let Some(b) = search.think(&board, &path, Duration::from_millis(60), 12) {
                let sc = search.last_score;
                best_white = if stm_white { sc } else { -sc };
                best_uci = display_uci_move(&board, b).to_string();
                best_san = san_of(&board, b);
            }
        }

        let mut child = board.clone();
        child.play_unchecked(mv);
        let mut cpath = path.clone();
        cpath.push(child.hash());
        let child_score = if legal_moves(&child).is_empty() {
            if child.checkers().is_empty() { 0 }
            else if child.side_to_move() == Color::White { -MATE + 10 } else { MATE - 10 }
        } else {
            match search.think(&child, &cpath, Duration::from_millis(30), 10) {
                Some(_) => {
                    let sc = search.last_score;
                    if child.side_to_move() == Color::White { sc } else { -sc }
                }
                None => best_white,
            }
        };

        let is_best = best_uci == *uci;
        let mut loss = 0;
        let mut tag = "engine";
        if is_player {
            loss = if is_best {
                0
            } else if stm_white {
                (best_white - child_score).max(0)
            } else {
                (child_score - best_white).max(0)
            };
            tag = if is_best { "best" }
                else if loss < 50 { "good" }
                else if loss < 100 { "inaccuracy" }
                else if loss < 250 { "mistake" }
                else { "blunder" };
            player_moves += 1;
            loss_sum += loss as i64;
            match tag {
                "best" => stats[0] += 1,
                "good" => stats[1] += 1,
                "inaccuracy" => stats[2] += 1,
                "mistake" => stats[3] += 1,
                _ => stats[4] += 1,
            }
        }

        let san = san_of(&board, mv);
        plies.push(format!(
            "{{\"i\":{},\"uci\":\"{}\",\"san\":\"{}\",\"cp\":{},\"loss\":{},\"tag\":\"{}\",\"player\":{},\"best\":\"{}\",\"bestSan\":\"{}\"}}",
            i, uci, json_esc(&san), child_score, loss, tag,
            if is_player { 1 } else { 0 }, json_esc(&best_uci), json_esc(&best_san)
        ));
        evals.push(child_score);
        board = child;
        path.push(board.hash());
    }
    search.quiet = false;

    if failed || plies.is_empty() {
        return "{\"error\":\"bad moves\"}".to_string();
    }
    let (accuracy, acpl) = if player_moves == 0 {
        (100.0, 0.0)
    } else {
        let avg = loss_sum as f64 / player_moves as f64;
        ((100.0 - avg / 8.0).clamp(0.0, 100.0), avg)
    };
    format!(
        "{{\"id\":\"{}\",\"opp\":\"{}\",\"color\":\"{}\",\"result\":\"{}\",\"reason\":\"{}\",\"accuracy\":{:.1},\"acpl\":{:.1},\"plies\":[{}],\"evals\":[{}],\"stats\":{{\"best\":{},\"good\":{},\"inaccuracy\":{},\"mistake\":{},\"blunder\":{}}}}}",
        game.id, game.opp, game.color, game.result, game.reason, accuracy, acpl,
        plies.join(","),
        evals.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","),
        stats[0], stats[1], stats[2], stats[3], stats[4]
    )
}


fn handle(stream: &mut TcpStream, search: &mut Search) -> io::Result<()> {
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf)?;
    let req = String::from_utf8_lossy(&buf[..n]).to_string();
    let first = req.lines().next().unwrap_or("");
    let target = first.split_whitespace().nth(1).unwrap_or("/");
    let (route, query) = match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target, ""),
    };
    match route {
        "/" | "/index.html" => respond(stream, "200 OK", "text/html; charset=utf-8", BOARD_HTML),
        "/api" => {
            let body = api(search, query);
            respond(stream, "200 OK", "application/json; charset=utf-8", &body)
        }
        "/api/save" => {
            let body = save_game(query);
            respond(stream, "200 OK", "application/json; charset=utf-8", &body)
        }
        "/games" => {
            let body = games_json();
            respond(stream, "200 OK", "application/json; charset=utf-8", &body)
        }
        "/config" => {
            let hosted = std::env::var("GREK_HOSTED").map(|v| v == "1").unwrap_or(false);
            let body = format!("{{\"hosted\":{}}}", hosted);
            respond(stream, "200 OK", "application/json; charset=utf-8", &body)
        }
        "/analyze" => {
            let body = analyze(search, query);
            respond(stream, "200 OK", "application/json; charset=utf-8", &body)
        }
        _ if route.starts_with("/voice/") => serve_voice(stream, &route[7..]),
        _ if route.starts_with("/img/") => serve_img(stream, &route[5..]),
        _ => respond(stream, "404 Not Found", "text/plain; charset=utf-8", "not found"),
    }
}

fn serve(host: &str, port: &str) {
    let addr = format!("{}:{}", host, port);
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => { eprintln!("Не удалось открыть {}: {}", addr, e); return; }
    };
    println!("grek слушает на http://{}", addr);
    let mut search = Search::new();
    for stream in listener.incoming() {
        if let Ok(mut s) = stream {
            let _ = handle(&mut s, &mut search);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("serve") {
        let port = args.get(2).cloned()
            .filter(|p| !p.is_empty())
            .or_else(|| std::env::var("PORT").ok())
            .unwrap_or_else(|| "8080".to_string());
        let host = args.get(3).cloned().unwrap_or_else(|| "127.0.0.1".to_string());
        serve(&host, &port);
        return;
    }
    uci_main();
}

fn uci_main() {
    let mut search = Search::new();
    let mut board = Board::default();
    let mut path = vec![board.hash()];

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line { Ok(l) => l, Err(_) => break };
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() { continue; }
        match tokens[0] {
            "uci" => {
                println!("id name grek v1");
                println!("id author me");
                println!("uciok");
            }
            "isready" => println!("readyok"),
            "ucinewgame" => {
                search.clear();
                board = Board::default();
                path = vec![board.hash()];
            }
            "position" => {
                if let Some((b, p)) = parse_position(&tokens) { board = b; path = p; }
            }
            "go" => {
                let (limit, depth) = time_for_go(&tokens, &board);
                match search.think(&board, &path, limit, depth) {
                    Some(m) => println!("bestmove {}", display_uci_move(&board, m)),
                    None => println!("bestmove 0000"),
                }
            }
            "quit" => break,
            _ => {}
        }
    }
}