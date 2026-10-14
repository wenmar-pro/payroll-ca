//! T4127 123rd edition, effective 2026-07-01.
//!
//! Source values (transcribed as Rust literals; the CSV files are not committed):
//! - `rates-income-thresholds-constants-26e.csv` (Table 8.1: A, R/V, K/KP), sha256
//!   e3a690ccf861c14654279d89cdeca8ec018506edbfc950027518bf16bf728090
//! - `other-rates-amounts-26e.csv` (Table 8.2: basic amounts, CEA, S2, V1 surtax), sha256
//!   d395b439d0e48846a05e5c578555d9a35e36aef58adbbf88b2eae2fcc2068bf8
//! - `cpp-qpp-ttl-01-26e.csv` (Table 8.3), sha256
//!   a8254ad90584f10a92f11fe5d34c9c6c42c856b686a8bc26c632886387e9ece4
//! - `cpp-qpp-br-01-26e.csv` (Table 8.4), sha256
//!   7d4f0f487f408590e2cbbb95ea65a5b5dd7052ae536da071a424ea147b3a5b87
//! - `cpp-qpp-addntl-01-26e.csv` (Table 8.5), sha256
//!   1362c4ca838bc9cab262eb6829904379adcd41fdd9ef5354496995d05aac9485
//! - `cpp-qpp-scnd-addntl-01-26e.csv` (Table 8.6), sha256
//!   aeeb9f2c042c11bb80ff744aa59edab56de43e523ef11109dd7985124a84448b
//! - `ei-01-26e.csv` (Table 8.7), sha256
//!   524162a6e1e62a7b2bdfe304a29f337273d03277519521a3cd380ec8e2fb0d1f
//!
//! Text rules that are not in the CSVs (January 2026 text, Chapters 4 to 7; July 2026
//! text for the BC reduction and the BC, NL and PE prorated items) are cited at each item.
//! The CSVs are the July 2026 release for Tables 8.1 and 8.2; the claim-code and CPP/EI
//! values are the same in both releases.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal::dec;

use super::{
    Bracket, CppRates, EiRates, Jurisdiction, OhpBand, RateSet, Reduction, Supplemental, Surtax,
};
use crate::Province;

fn br(threshold: Decimal, rate: Decimal, constant: Decimal) -> Bracket {
    Bracket {
        threshold,
        rate,
        constant,
    }
}

fn jur(brackets: Vec<Bracket>, basic_amount: Decimal) -> Jurisdiction {
    Jurisdiction {
        brackets,
        basic_amount,
        cea: None,
        reduction: Reduction::None,
        surtax: None,
        supplemental: None,
    }
}

/// Build the 123rd edition rate set.
pub fn rate_set() -> RateSet {
    // Federal: Table 8.1 row "Federal" (A, R, K); Table 8.2 BPAF (text: BPAF = $16,452
    // maximum for 2026, Chapter 2) and CEA 1,501.
    let mut federal = jur(
        vec![
            br(dec!(0), dec!(0.14), dec!(0)),
            br(dec!(58523), dec!(0.205), dec!(3804)),
            br(dec!(117045), dec!(0.26), dec!(10241)),
            br(dec!(181440), dec!(0.29), dec!(15685)),
            br(dec!(258482), dec!(0.33), dec!(26024)),
        ],
        dec!(16452),
    );
    federal.cea = Some(dec!(1501));

    let mut provinces = Vec::new();

    // Alberta: K5P supplemental credit (text, Step 5, Alberta).
    let mut ab = jur(
        vec![
            br(dec!(0), dec!(0.08), dec!(0)),
            br(dec!(61200), dec!(0.10), dec!(1224)),
            br(dec!(154259), dec!(0.12), dec!(4309)),
            br(dec!(185111), dec!(0.13), dec!(6160)),
            br(dec!(246813), dec!(0.14), dec!(8628)),
            br(dec!(370220), dec!(0.15), dec!(12331)),
        ],
        dec!(22769),
    );
    ab.supplemental = Some(Supplemental {
        threshold: dec!(4896),
        rate: dec!(0.25),
    });
    provinces.push((Province::AB, ab));

    // British Columbia: July 2026 text, Option 1 K1P = 0.0614 and S (basic reduction 805,
    // phase-out from 25,570 at 3.56% to 44,952).
    let mut bc = jur(
        vec![
            br(dec!(0), dec!(0.0614), dec!(0)),
            br(dec!(50363), dec!(0.077), dec!(786)),
            br(dec!(100728), dec!(0.105), dec!(3606)),
            br(dec!(115648), dec!(0.1229), dec!(5676)),
            br(dec!(140430), dec!(0.147), dec!(9061)),
            br(dec!(190405), dec!(0.168), dec!(13059)),
            br(dec!(265545), dec!(0.205), dec!(22884)),
        ],
        dec!(13216),
    );
    bc.reduction = Reduction::BritishColumbia {
        s2: dec!(805),
        phase_start: dec!(25570),
        phase_end: dec!(44952),
        phase_rate: dec!(0.0356),
    };
    provinces.push((Province::BC, bc));

    provinces.push((
        Province::MB,
        jur(
            vec![
                br(dec!(0), dec!(0.108), dec!(0)),
                br(dec!(47000), dec!(0.1275), dec!(917)),
                br(dec!(100000), dec!(0.174), dec!(5567)),
            ],
            dec!(15780),
        ),
    ));

    provinces.push((
        Province::NB,
        jur(
            vec![
                br(dec!(0), dec!(0.094), dec!(0)),
                br(dec!(52333), dec!(0.14), dec!(2407)),
                br(dec!(104666), dec!(0.16), dec!(4501)),
                br(dec!(193861), dec!(0.195), dec!(11286)),
            ],
            dec!(13664),
        ),
    ));

    // Newfoundland and Labrador: K1P 0.0870 and prorated BPA of 15,000 (July text).
    provinces.push((
        Province::NL,
        jur(
            vec![
                br(dec!(0), dec!(0.087), dec!(0)),
                br(dec!(44678), dec!(0.145), dec!(2591)),
                br(dec!(89354), dec!(0.158), dec!(3753)),
                br(dec!(159528), dec!(0.178), dec!(6943)),
                br(dec!(223340), dec!(0.198), dec!(11410)),
                br(dec!(285319), dec!(0.208), dec!(14263)),
                br(dec!(570638), dec!(0.213), dec!(17117)),
                br(dec!(1141275), dec!(0.218), dec!(22823)),
            ],
            dec!(15000),
        ),
    ));

    provinces.push((
        Province::NS,
        jur(
            vec![
                br(dec!(0), dec!(0.0879), dec!(0)),
                br(dec!(30995), dec!(0.1495), dec!(1909)),
                br(dec!(61991), dec!(0.1667), dec!(2976)),
                br(dec!(97417), dec!(0.175), dec!(3784)),
                br(dec!(157124), dec!(0.21), dec!(9283)),
            ],
            dec!(11932),
        ),
    ));

    provinces.push((
        Province::NT,
        jur(
            vec![
                br(dec!(0), dec!(0.059), dec!(0)),
                br(dec!(53003), dec!(0.086), dec!(1431)),
                br(dec!(106009), dec!(0.122), dec!(5247)),
                br(dec!(172346), dec!(0.1405), dec!(8436)),
            ],
            dec!(18198),
        ),
    ));

    provinces.push((
        Province::NU,
        jur(
            vec![
                br(dec!(0), dec!(0.04), dec!(0)),
                br(dec!(55801), dec!(0.07), dec!(1674)),
                br(dec!(111602), dec!(0.09), dec!(3906)),
                br(dec!(181439), dec!(0.115), dec!(8442)),
            ],
            dec!(19659),
        ),
    ));

    // Ontario: surtax V1 (Table 8.2, T4 to V1 5,818 / 7,446; rates 0.20 / 0.36) and the
    // tax reduction S with S2 = 300 (Table 8.2). OHP bands V2 are in `ohp` below.
    let mut on = jur(
        vec![
            br(dec!(0), dec!(0.0505), dec!(0)),
            br(dec!(53891), dec!(0.0915), dec!(2210)),
            br(dec!(107785), dec!(0.1116), dec!(4376)),
            br(dec!(150000), dec!(0.1216), dec!(5876)),
            br(dec!(220000), dec!(0.1316), dec!(8076)),
        ],
        dec!(12989),
    );
    on.surtax = Some(Surtax {
        first_threshold: dec!(5818),
        first_rate: dec!(0.20),
        second_threshold: dec!(7446),
        second_rate: dec!(0.36),
    });
    on.reduction = Reduction::Ontario { s2: dec!(300) };
    provinces.push((Province::ON, on));

    // Prince Edward Island: July text, K1P 0.0950, prorated top rate 21% (CSV 0.2100).
    provinces.push((
        Province::PE,
        jur(
            vec![
                br(dec!(0), dec!(0.095), dec!(0)),
                br(dec!(33928), dec!(0.1347), dec!(1347)),
                br(dec!(65820), dec!(0.166), dec!(3407)),
                br(dec!(106890), dec!(0.1762), dec!(4497)),
                br(dec!(142520), dec!(0.19), dec!(6464)),
                br(dec!(200000), dec!(0.21), dec!(10464)),
            ],
            dec!(15000),
        ),
    ));

    provinces.push((
        Province::SK,
        jur(
            vec![
                br(dec!(0), dec!(0.105), dec!(0)),
                br(dec!(54532), dec!(0.125), dec!(1091)),
                br(dec!(155805), dec!(0.145), dec!(4207)),
            ],
            dec!(20381),
        ),
    ));

    // Yukon: BPAYT mirrors the federal BPAF (Chapter 2); K4P uses the CEA (Step 5, Yukon).
    let mut yt = jur(
        vec![
            br(dec!(0), dec!(0.064), dec!(0)),
            br(dec!(58523), dec!(0.09), dec!(1522)),
            br(dec!(117045), dec!(0.109), dec!(3745)),
            br(dec!(181440), dec!(0.128), dec!(7193)),
            br(dec!(500000), dec!(0.15), dec!(18193)),
        ],
        dec!(16452),
    );
    yt.cea = Some(dec!(1501));
    provinces.push((Province::YT, yt));

    RateSet {
        edition: 123,
        effective: NaiveDate::from_ymd_opt(2026, 7, 1).expect("valid date"),
        valid_until: NaiveDate::from_ymd_opt(2026, 12, 31).expect("valid date"),
        federal,
        provinces,
        cpp: CppRates {
            ympe: dec!(74600),
            yampe: dec!(85000),
            basic_exemption: dec!(3500),
            base_rate: dec!(0.0495),
            total_rate: dec!(0.0595),
            max_base: dec!(3519.45),
            max_total: dec!(4230.45),
            second_rate: dec!(0.04),
            max_second: dec!(416),
        },
        ei: EiRates {
            max_insurable: dec!(68900),
            rate: dec!(0.0163),
            max_employee: dec!(1123.07),
            employer_multiplier: dec!(1.4),
        },
        // Table 8.1 Ontario Health Premium; Option 1 text, Step 5, Ontario.
        ohp: vec![
            OhpBand {
                lower: dec!(20000),
                upper: Some(dec!(36000)),
                base: dec!(0),
                rate: dec!(0.06),
                cap: dec!(300),
            },
            OhpBand {
                lower: dec!(36000),
                upper: Some(dec!(48000)),
                base: dec!(300),
                rate: dec!(0.06),
                cap: dec!(450),
            },
            OhpBand {
                lower: dec!(48000),
                upper: Some(dec!(72000)),
                base: dec!(450),
                rate: dec!(0.25),
                cap: dec!(600),
            },
            OhpBand {
                lower: dec!(72000),
                upper: Some(dec!(200000)),
                base: dec!(600),
                rate: dec!(0.25),
                cap: dec!(750),
            },
            OhpBand {
                lower: dec!(200000),
                upper: None,
                base: dec!(750),
                rate: dec!(0.25),
                cap: dec!(900),
            },
        ],
    }
}
