use clap::Parser;

const MAX_CARDS: usize = 12;

const NUM_RANKS: u8 = 13;

type RankCounts = [u8; NUM_RANKS as usize];

const NUM_SUITS: u8 = 4;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct Card {
    suit: u8,
    rank: u8,
}

fn rank_counts(cards: &[Card]) -> RankCounts {
    let mut ret = RankCounts::default();
    for c in cards {
        ret[c.rank as usize] += 1;
    }
    ret
}

type Ranks = [u8; (NUM_RANKS + 1) as usize];

fn ranks_for_straight(cards: &[Card]) -> Ranks {
    let mut ret = Ranks::default();
    for c in cards {
        ret[c.rank as usize + 1] = 1;
    }
    ret[0] = *ret.last().unwrap();
    ret
}

fn suit_counts(cards: &[Card]) -> RankCounts {
    let mut ret = RankCounts::default();
    for c in cards {
        ret[c.suit as usize] += 1;
    }
    ret
}

fn is_n_of_a_kind(cards: &[Card], n: u8, num_jokers: u8) -> bool {
    let mut counts = <[u8; NUM_RANKS as usize]>::default();
    for &c in cards {
        let count = &mut counts[c.rank as usize];
        *count += 1;
        if *count + num_jokers >= n {
            return true;
        }
    }
    num_jokers >= n
}

fn is_two_pair(cards: &[Card], mut num_jokers: u8) -> bool {
    let mut num_pairs = 0;
    for i in rank_counts(cards) {
        if i % 2 == 1 && num_jokers > 0 {
            num_jokers -= 1;
            num_pairs += 1;
        }
        num_pairs += i / 2;
    }
    num_pairs + num_jokers / 2 >= 2
}

fn is_full_house(cards: &[Card], mut num_jokers: u8) -> bool {
    let mut fill_with_jokers = |val: &mut u8, fill_to: u8| -> bool {
        if *val >= fill_to {
            return true;
        }
        if *val + num_jokers < fill_to {
            return false;
        }
        num_jokers -= fill_to - *val;
        *val = fill_to;
        true
    };
    let mut rank_counts = rank_counts(cards);
    // FIXME: no need to sort, just find two largest values
    rank_counts.sort_by(|a, b| b.cmp(a));
    if !fill_with_jokers(&mut rank_counts[0], 3) {
        return false;
    }
    rank_counts[0] -= 3;
    if fill_with_jokers(&mut rank_counts[0], 2) {
        return true;
    }
    fill_with_jokers(&mut rank_counts[1], 2)
}

fn is_flush(cards: &[Card], num_jokers: u8) -> bool {
    suit_counts(cards).iter().any(|&c| c + num_jokers >= 5)
}

fn is_straight(cards: &[Card], num_jokers: u8) -> bool {
    let ranks = ranks_for_straight(cards);
    let mut window_sum = ranks.iter().take(5).sum::<u8>();
    if window_sum + num_jokers == 5 {
        return true;
    }
    for i in 5..ranks.len() {
        window_sum -= ranks[i - 5];
        window_sum += ranks[i];
        if window_sum + num_jokers == 5 {
            return true;
        }
    }
    false
}

fn is_straight_flush(cards: &[Card], num_jokers: u8) -> bool {
    let mut cards_by_suit = <[arrayvec::ArrayVec<Card, MAX_CARDS>; NUM_SUITS as usize]>::default();

    for &c in cards {
        cards_by_suit[c.suit as usize].push(c);
    }

    cards_by_suit
        .iter()
        .any(|cards| is_straight(cards, num_jokers))
}

fn is_flush_house(cards: &[Card], num_jokers: u8) -> bool {
    let mut cards_by_suit = <[arrayvec::ArrayVec<Card, MAX_CARDS>; NUM_SUITS as usize]>::default();

    for &c in cards {
        cards_by_suit[c.suit as usize].push(c);
    }

    cards_by_suit
        .iter()
        .any(|cards| is_full_house(cards, num_jokers))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CardOrJoker {
    Card(Card),
    Joker,
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, default_value_t = 7)]
    cards: usize,

    #[arg(long, default_value_t = 1)]
    decks: usize,

    #[arg(long, default_value_t = 0)]
    jokers: u8,
}

fn confidence_interval(total_iters: u64, num_true: u64) -> (f64, f64) {
    let p = num_true as f64 / total_iters as f64;
    // 99.73% confidence interval according to https://sigmazone.com/binomial-confidence-intervals/
    let ci = 3.0 * (p * (1.0 - p) / total_iters as f64).sqrt();
    (p, ci)
}

struct HandCount {
    name: &'static str,
    count: u64,
    func: fn(&[Card], u8) -> bool,
}

impl HandCount {
    fn new(name: &'static str, func: fn(&[Card], u8) -> bool) -> Self {
        Self {
            name,
            count: 0,
            func,
        }
    }

    // TODO: write tests
    fn overlap(&self, total_iters: u64, other: &HandCount) -> bool {
        if self.count == 0 || other.count == 0 {
            return false;
        }
        let ci1 = confidence_interval(total_iters, self.count);
        let ci2 = confidence_interval(total_iters, other.count);
        let ci1_start = ci1.0 - ci1.1;
        let ci1_end = ci1.0 + ci1.1;
        let ci2_start = ci2.0 - ci2.1;
        let ci2_end = ci2.0 + ci2.1;
        ci1_start <= ci2_end && ci2_start <= ci1_end
    }
}

fn main() {
    use rand::seq::SliceRandom;

    let args = Args::parse();

    if args.cards > MAX_CARDS {
        println!("Does not support more than {} cards", MAX_CARDS);
        std::process::exit(1);
    }

    let mut rng = rand::thread_rng();
    let mut deck = Vec::new();
    for _ in 0..args.decks {
        for suit in 0..NUM_SUITS {
            for rank in 0..NUM_RANKS {
                deck.push(CardOrJoker::Card(Card { suit, rank }));
            }
        }
    }
    for _ in 0..args.jokers {
        deck.push(CardOrJoker::Joker);
    }

    let mut counts = [
        HandCount::new("Pair", |cards, num_jokers| {
            is_n_of_a_kind(cards, 2, num_jokers)
        }),
        HandCount::new("3oak", |cards, num_jokers| {
            is_n_of_a_kind(cards, 3, num_jokers)
        }),
        HandCount::new("4oak", |cards, num_jokers| {
            is_n_of_a_kind(cards, 4, num_jokers)
        }),
        HandCount::new("5oak", |cards, num_jokers| {
            is_n_of_a_kind(cards, 5, num_jokers)
        }),
        HandCount::new("2 pair", is_two_pair),
        HandCount::new("Straight", is_straight),
        HandCount::new("Flush", is_flush),
        HandCount::new("Full House", is_full_house),
        HandCount::new("Strt Flush", is_straight_flush),
        HandCount::new("Flush House", is_flush_house),
    ];

    let mut num_iters: u64 = 0;

    loop {
        for _ in 0..100000 {
            let cards_or_jokers = deck
                .choose_multiple(&mut rng, args.cards)
                .copied()
                .collect::<arrayvec::ArrayVec<CardOrJoker, MAX_CARDS>>();
            let num_jokers = cards_or_jokers
                .iter()
                .filter(|&&coj| coj == CardOrJoker::Joker)
                .count() as u8;
            let cards = cards_or_jokers
                .iter()
                .filter_map(|coj| match coj {
                    CardOrJoker::Card(c) => Some(*c),
                    CardOrJoker::Joker => None,
                })
                .collect::<arrayvec::ArrayVec<Card, MAX_CARDS>>();
            for c in &mut counts {
                if (c.func)(&cards, num_jokers) {
                    c.count += 1;
                }
            }
            num_iters += 1;
        }
        let mut has_overlap = false;
        'outer: for (idx, c1) in counts.iter().enumerate() {
            for c2 in counts.iter().skip(idx + 1) {
                has_overlap = c1.overlap(num_iters, c2);
                if has_overlap {
                    break 'outer;
                }
            }
        }
        println!("{num_iters} iterations...");
        if !has_overlap {
            break;
        }
    }
    println!("--------------");
    println!("(no overlapping 99% confidence intervals)");
    println!("total iterations: {num_iters}");
    let max_str_len = counts.iter().map(|c| c.name.len()).max().unwrap();
    counts.sort_by_key(|c| (c.count, c.name));
    counts.reverse();
    for c in counts {
        println!(
            "{name: >width$}: {p:.6} ({count})",
            name = c.name,
            width = max_str_len,
            p = (c.count as f64 / num_iters as f64),
            count = c.count,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const R2: u8 = 0;
    const R3: u8 = 1;
    const R4: u8 = 2;
    const R5: u8 = 3;
    const R6: u8 = 4;
    const R7: u8 = 5;
    const R8: u8 = 6;
    const R9: u8 = 7;
    const R10: u8 = 8;
    const RJ: u8 = 9;
    const RQ: u8 = 10;
    const RK: u8 = 11;
    const RA: u8 = 12;

    #[test]
    fn test_rank_counts() {
        assert_eq!(RankCounts::default(), rank_counts(&[]));

        {
            let mut expected = RankCounts::default();
            expected[1] = 2;
            expected[3] = 1;
            assert_eq!(
                expected,
                rank_counts(&[
                    Card { suit: 0, rank: 1 },
                    Card { suit: 0, rank: 1 },
                    Card { suit: 2, rank: 3 }
                ])
            )
        }
    }

    #[test]
    fn test_ranks_for_straight() {
        assert_eq!(Ranks::default(), ranks_for_straight(&[]));

        {
            let mut expected = Ranks::default();
            expected[2] = 1;
            expected[4] = 1;
            assert_eq!(
                expected,
                ranks_for_straight(&[
                    Card { suit: 0, rank: 1 },
                    Card { suit: 0, rank: 1 },
                    Card { suit: 2, rank: 3 }
                ])
            )
        }

        {
            let mut expected = Ranks::default();
            expected[0] = 1;
            expected[1] = 1;
            expected[3] = 1;
            expected[13] = 1;
            assert_eq!(
                expected,
                ranks_for_straight(&[
                    Card { suit: 0, rank: R2 },
                    Card { suit: 0, rank: RA },
                    Card { suit: 2, rank: R4 }
                ])
            )
        }
    }

    #[test]
    fn test_suit_counts() {
        assert_eq!(RankCounts::default(), suit_counts(&[]));

        {
            let mut expected = RankCounts::default();
            expected[1] = 2;
            expected[3] = 1;
            assert_eq!(
                expected,
                suit_counts(&[
                    Card { suit: 1, rank: 0 },
                    Card { suit: 1, rank: 0 },
                    Card { suit: 3, rank: 2 }
                ])
            )
        }
    }

    #[test]
    fn test_is_n_of_a_kind() {
        assert!(is_n_of_a_kind(&[], 0, 0));
        assert!(!is_n_of_a_kind(&[], 1, 0));
        assert!(is_n_of_a_kind(&[Card { suit: 0, rank: 1 }], 1, 0));

        assert!(!is_n_of_a_kind(
            &[Card { suit: 1, rank: 0 }, Card { suit: 0, rank: 1 }],
            2,
            0
        ));
        assert!(is_n_of_a_kind(
            &[Card { suit: 1, rank: 1 }, Card { suit: 1, rank: 1 }],
            2,
            0
        ));
        assert!(is_n_of_a_kind(
            &[Card { suit: 0, rank: 1 }, Card { suit: 1, rank: 1 }],
            2,
            0
        ));

        assert!(!is_n_of_a_kind(
            &[Card { suit: 0, rank: 1 }, Card { suit: 1, rank: 1 }],
            3,
            0
        ));
        assert!(is_n_of_a_kind(
            &[
                Card { suit: 0, rank: 1 },
                Card { suit: 0, rank: 1 },
                Card { suit: 1, rank: 1 },
            ],
            3,
            0
        ));
        assert!(is_n_of_a_kind(
            &[
                Card { suit: 0, rank: 1 },
                Card { suit: 0, rank: 1 },
                Card { suit: 1, rank: 1 },
                Card { suit: 1, rank: 2 },
            ],
            3,
            0
        ));

        assert!(!is_n_of_a_kind(&[], 2, 1));
        assert!(is_n_of_a_kind(&[], 2, 2));
        assert!(is_n_of_a_kind(&[Card { suit: 1, rank: 2 },], 2, 1));
        assert!(!is_n_of_a_kind(
            &[Card { suit: 1, rank: 2 }, Card { suit: 2, rank: 3 },],
            3,
            1
        ));
        assert!(is_n_of_a_kind(
            &[Card { suit: 1, rank: 3 }, Card { suit: 2, rank: 3 },],
            3,
            1
        ));
    }

    #[test]
    fn test_is_two_pair() {
        assert!(!is_two_pair(&[], 0));
        assert!(!is_two_pair(&[Card { suit: 0, rank: 0 }], 0));
        assert!(!is_two_pair(
            &[Card { suit: 0, rank: 0 }, Card { suit: 0, rank: 0 },],
            0
        ));
        assert!(!is_two_pair(
            &[
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
            ],
            0
        ));
        assert!(is_two_pair(
            &[
                Card { suit: 0, rank: 1 },
                Card { suit: 1, rank: 1 },
                Card { suit: 2, rank: 0 },
                Card { suit: 3, rank: 0 },
            ],
            0
        ));
        assert!(is_two_pair(
            &[
                Card { suit: 0, rank: 0 },
                Card { suit: 2, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 2, rank: 0 },
            ],
            0
        ));
        assert!(is_two_pair(
            &[
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
            ],
            0
        ));

        assert!(!is_two_pair(&[], 3));
        assert!(is_two_pair(&[], 4));
        assert!(is_two_pair(&[Card { suit: 0, rank: 0 },], 3));
        assert!(is_two_pair(
            &[Card { suit: 1, rank: 1 }, Card { suit: 0, rank: 0 },],
            2
        ));
        assert!(is_two_pair(
            &[Card { suit: 0, rank: 0 }, Card { suit: 0, rank: 0 },],
            2
        ));
        assert!(is_two_pair(
            &[
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 1, rank: 1 },
            ],
            1
        ));
        assert!(!is_two_pair(
            &[Card { suit: 0, rank: 0 }, Card { suit: 0, rank: 0 },],
            1
        ));
    }

    #[test]
    fn test_is_full_house() {
        assert!(!is_full_house(&[], 0));
        assert!(!is_full_house(
            &[
                Card { suit: 0, rank: 1 },
                Card { suit: 1, rank: 1 },
                Card { suit: 2, rank: 0 },
                Card { suit: 3, rank: 0 },
            ],
            0
        ));
        assert!(!is_full_house(
            &[
                Card { suit: 0, rank: 1 },
                Card { suit: 1, rank: 1 },
                Card { suit: 2, rank: 0 },
                Card { suit: 3, rank: 0 },
                Card { suit: 3, rank: 2 },
            ],
            0
        ));
        assert!(is_full_house(
            &[
                Card { suit: 0, rank: 1 },
                Card { suit: 1, rank: 1 },
                Card { suit: 2, rank: 0 },
                Card { suit: 3, rank: 0 },
                Card { suit: 3, rank: 0 },
            ],
            0
        ));
        assert!(is_full_house(
            &[
                Card { suit: 0, rank: 0 },
                Card { suit: 1, rank: 0 },
                Card { suit: 2, rank: 0 },
                Card { suit: 3, rank: 0 },
                Card { suit: 3, rank: 0 },
            ],
            0
        ));
        assert!(is_full_house(&[], 5));
        assert!(!is_full_house(&[], 4));
        assert!(is_full_house(&[Card { suit: 0, rank: 0 },], 4));
        assert!(is_full_house(
            &[
                Card { suit: 0, rank: 0 },
                Card { suit: 1, rank: 1 },
                Card { suit: 1, rank: 1 },
            ],
            2
        ));
        assert!(is_full_house(
            &[Card { suit: 0, rank: 0 }, Card { suit: 1, rank: 1 },],
            3
        ));
        assert!(is_full_house(
            &[
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 1, rank: 1 },
                Card { suit: 1, rank: 1 },
            ],
            1
        ));
        assert!(is_full_house(
            &[
                Card { suit: 0, rank: 0 },
                Card { suit: 1, rank: 1 },
                Card { suit: 1, rank: 1 },
                Card { suit: 1, rank: 1 },
            ],
            1
        ));
        assert!(is_full_house(
            &[
                Card { suit: 1, rank: 1 },
                Card { suit: 1, rank: 1 },
                Card { suit: 1, rank: 1 },
                Card { suit: 1, rank: 1 },
            ],
            1
        ));
    }

    #[test]
    fn test_is_flush() {
        assert!(!is_flush(&[], 0));
        assert!(!is_flush(&[Card { suit: 0, rank: 0 },], 0));
        assert!(!is_flush(
            &[
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
            ],
            0
        ));
        assert!(is_flush(
            &[
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 1 },
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
            ],
            0
        ));
        assert!(!is_flush(
            &[
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 1, rank: 0 },
            ],
            0
        ));
        assert!(!is_flush(&[], 4));
        assert!(is_flush(&[], 5));
        assert!(is_flush(
            &[
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 1, rank: 0 },
            ],
            1
        ));
        assert!(is_flush(
            &[
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 1, rank: 0 },
            ],
            2
        ));
        assert!(!is_flush(
            &[
                Card { suit: 0, rank: 0 },
                Card { suit: 0, rank: 0 },
                Card { suit: 1, rank: 0 },
            ],
            2
        ));
    }

    #[test]
    fn test_is_straight() {
        assert!(!is_straight(&[], 0));
        assert!(!is_straight(&[Card { suit: 0, rank: 0 },], 0));

        assert!(!is_straight(
            &[
                Card { suit: 0, rank: 2 },
                Card { suit: 0, rank: 3 },
                Card { suit: 0, rank: 4 },
                Card { suit: 0, rank: 5 },
            ],
            0
        ));
        assert!(is_straight(
            &[
                Card { suit: 1, rank: R2 },
                Card { suit: 1, rank: R3 },
                Card { suit: 1, rank: R4 },
                Card { suit: 1, rank: R5 },
                Card { suit: 1, rank: R6 },
            ],
            0
        ));
        assert!(is_straight(
            &[
                Card { suit: 2, rank: R2 },
                Card { suit: 3, rank: R3 },
                Card { suit: 0, rank: R4 },
                Card { suit: 0, rank: R5 },
                Card { suit: 1, rank: R6 },
            ],
            0
        ));
        assert!(is_straight(
            &[
                Card { suit: 0, rank: R10 },
                Card { suit: 0, rank: RJ },
                Card { suit: 0, rank: RQ },
                Card { suit: 0, rank: RK },
                Card { suit: 0, rank: RA },
            ],
            0
        ));
        assert!(is_straight(
            &[
                Card { suit: 0, rank: RA },
                Card { suit: 0, rank: R2 },
                Card { suit: 0, rank: R3 },
                Card { suit: 0, rank: R4 },
                Card { suit: 0, rank: R5 },
            ],
            0
        ));
        assert!(!is_straight(
            &[
                Card { suit: 0, rank: RK },
                Card { suit: 0, rank: RA },
                Card { suit: 0, rank: R2 },
                Card { suit: 0, rank: R3 },
                Card { suit: 0, rank: R4 },
            ],
            0
        ));
        assert!(!is_straight(&[], 4));
        assert!(is_straight(&[], 5));
        assert!(is_straight(
            &[
                Card { suit: 0, rank: RA },
                Card { suit: 0, rank: R2 },
                Card { suit: 0, rank: R3 },
                Card { suit: 0, rank: R4 },
            ],
            1
        ));
        assert!(is_straight(
            &[
                Card { suit: 0, rank: R2 },
                Card { suit: 0, rank: R3 },
                Card { suit: 0, rank: R4 },
                Card { suit: 0, rank: R5 },
            ],
            1
        ));
        assert!(is_straight(
            &[
                Card { suit: 0, rank: R2 },
                Card { suit: 0, rank: R3 },
                Card { suit: 0, rank: R4 },
            ],
            2
        ));
        assert!(is_straight(
            &[
                Card { suit: 0, rank: R2 },
                Card { suit: 0, rank: R4 },
                Card { suit: 0, rank: R5 },
            ],
            2
        ));
        assert!(is_straight(
            &[
                Card { suit: 0, rank: R2 },
                Card { suit: 0, rank: R4 },
                Card { suit: 0, rank: R6 },
            ],
            2
        ));
        assert!(is_straight(
            &[Card { suit: 0, rank: R2 }, Card { suit: 0, rank: R6 },],
            3
        ));
        assert!(is_straight(
            &[Card { suit: 0, rank: R3 }, Card { suit: 0, rank: R6 },],
            3
        ));
        assert!(!is_straight(
            &[Card { suit: 0, rank: R3 }, Card { suit: 0, rank: R4 },],
            2
        ));
        assert!(is_straight(
            &[
                Card { suit: 0, rank: R10 },
                Card { suit: 0, rank: RJ },
                Card { suit: 0, rank: RK },
                Card { suit: 0, rank: RA },
            ],
            1
        ));
        assert!(is_straight(
            &[
                Card { suit: 0, rank: R10 },
                Card { suit: 0, rank: RJ },
                Card { suit: 0, rank: RQ },
                Card { suit: 0, rank: RK },
            ],
            1
        ));
    }

    #[test]
    fn test_is_straight_flush() {
        assert!(!is_straight_flush(&[], 0));
        assert!(!is_straight_flush(&[Card { suit: 0, rank: 0 },], 0));

        assert!(!is_straight_flush(
            &[
                Card { suit: 0, rank: 2 },
                Card { suit: 0, rank: 3 },
                Card { suit: 0, rank: 4 },
                Card { suit: 0, rank: 5 },
            ],
            0
        ));
        assert!(is_straight_flush(
            &[
                Card { suit: 1, rank: R2 },
                Card { suit: 1, rank: R3 },
                Card { suit: 1, rank: R4 },
                Card { suit: 1, rank: R5 },
                Card { suit: 1, rank: R6 },
            ],
            0
        ));
        assert!(!is_straight_flush(
            &[
                Card { suit: 2, rank: R2 },
                Card { suit: 3, rank: R3 },
                Card { suit: 0, rank: R4 },
                Card { suit: 0, rank: R5 },
                Card { suit: 1, rank: R6 },
            ],
            0
        ));
        assert!(is_straight_flush(
            &[
                Card { suit: 0, rank: R10 },
                Card { suit: 0, rank: RJ },
                Card { suit: 0, rank: RQ },
                Card { suit: 0, rank: RK },
                Card { suit: 0, rank: RA },
            ],
            0
        ));
        assert!(is_straight_flush(
            &[
                Card { suit: 0, rank: RA },
                Card { suit: 0, rank: R2 },
                Card { suit: 0, rank: R3 },
                Card { suit: 0, rank: R4 },
                Card { suit: 0, rank: R5 },
            ],
            0
        ));
        assert!(!is_straight_flush(
            &[
                Card { suit: 0, rank: RK },
                Card { suit: 0, rank: RA },
                Card { suit: 0, rank: R2 },
                Card { suit: 0, rank: R3 },
                Card { suit: 0, rank: R4 },
            ],
            0
        ));
        assert!(is_straight_flush(
            &[
                Card { suit: 1, rank: R4 },
                Card { suit: 0, rank: R5 },
                Card { suit: 0, rank: R6 },
                Card { suit: 0, rank: R7 },
                Card { suit: 0, rank: R8 },
                Card { suit: 0, rank: R9 },
            ],
            0
        ));
        assert!(!is_straight_flush(&[], 4));
        assert!(is_straight_flush(&[], 5));
        assert!(is_straight_flush(
            &[
                Card { suit: 0, rank: R5 },
                Card { suit: 0, rank: R6 },
                Card { suit: 1, rank: R7 },
                Card { suit: 0, rank: R8 },
                Card { suit: 0, rank: R9 },
            ],
            1
        ));
        assert!(!is_straight_flush(
            &[
                Card { suit: 0, rank: R5 },
                Card { suit: 0, rank: R6 },
                Card { suit: 1, rank: R7 },
                Card { suit: 1, rank: R8 },
                Card { suit: 0, rank: R9 },
            ],
            1
        ));
        assert!(!is_straight_flush(
            &[
                Card { suit: 0, rank: R5 },
                Card { suit: 0, rank: R6 },
                Card { suit: 1, rank: R9 },
            ],
            2
        ));
        assert!(is_straight_flush(
            &[
                Card { suit: 0, rank: R5 },
                Card { suit: 0, rank: R6 },
                Card { suit: 0, rank: R9 },
            ],
            2
        ));
    }

    #[test]
    fn test_is_flush_house() {
        assert!(!is_flush_house(&[], 0));
        assert!(is_flush_house(
            &[
                Card { suit: 0, rank: 1 },
                Card { suit: 0, rank: 1 },
                Card { suit: 0, rank: 1 },
                Card { suit: 0, rank: 1 },
                Card { suit: 0, rank: 1 },
            ],
            0
        ));
        assert!(is_flush_house(
            &[
                Card { suit: 0, rank: 1 },
                Card { suit: 0, rank: 1 },
                Card { suit: 0, rank: 2 },
                Card { suit: 0, rank: 2 },
                Card { suit: 0, rank: 2 },
            ],
            0
        ));
        assert!(!is_flush_house(
            &[
                Card { suit: 0, rank: 1 },
                Card { suit: 0, rank: 1 },
                Card { suit: 0, rank: 2 },
                Card { suit: 1, rank: 2 },
                Card { suit: 0, rank: 2 },
            ],
            0
        ));

        assert!(!is_flush_house(&[], 4));
        assert!(is_flush_house(&[], 5));
        assert!(is_flush_house(
            &[
                Card { suit: 0, rank: 1 },
                Card { suit: 0, rank: 1 },
                Card { suit: 0, rank: 2 },
                Card { suit: 0, rank: 2 },
            ],
            1
        ));
        assert!(!is_flush_house(
            &[
                Card { suit: 1, rank: 1 },
                Card { suit: 0, rank: 1 },
                Card { suit: 0, rank: 2 },
                Card { suit: 0, rank: 2 },
            ],
            1
        ));
        assert!(is_flush_house(
            &[
                Card { suit: 0, rank: 2 },
                Card { suit: 0, rank: 2 },
                Card { suit: 0, rank: 2 },
            ],
            2
        ));
        assert!(is_flush_house(
            &[
                Card { suit: 0, rank: 1 },
                Card { suit: 0, rank: 2 },
                Card { suit: 0, rank: 2 },
            ],
            2
        ));
    }
}
