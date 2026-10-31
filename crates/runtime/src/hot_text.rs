//! Hot code on huge pages. A program linked with a symbol ordering file (`scripts/hot-text-order.py`,
//! see docs/performance.md, "Hot code together") has the functions that a profile of it found hot
//! at the start of its code, in that file's order, between `j2r_hot_text_start` and
//! `j2r_hot_text_end`, which the file names first and last. Before `main` runs, the 2 MB blocks of
//! the executable's code that hold that range are made into huge pages (`MADV_COLLAPSE`), so the
//! hot code takes a few entries of the processor's instruction TLB instead of one per 4 KB page.
//! The program must be linked with its segments aligned to 2 MB (`-z max-page-size=0x200000
//! -z separate-loadable-segments`) for its code's 2 MB blocks to be blocks of its file too.
//! Without an ordering file the two markers lie next to each other, and nothing is done.

/// The first function of the hot range. Its body differs from `j2r_hot_text_end`'s, so that the
/// two are not merged into one function.
#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn j2r_hot_text_start() -> u32 {
    0x686f_7431
}

/// The last function of the hot range.
#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn j2r_hot_text_end() -> u32 {
    0x686f_7432
}

/// Below this distance between the markers, the program was linked without an ordering file.
const ORDERED_ABOVE: usize = 64 << 10;
const HUGE_PAGE: usize = 2 << 20;

#[cfg(target_os = "linux")]
mod collapse {
    use super::*;

    unsafe extern "C" {
        fn madvise(addr: *mut u8, length: usize, advice: i32) -> i32;
    }
    const MADV_HUGEPAGE: i32 = 14;
    const MADV_COLLAPSE: i32 = 25;

    /// Run before `main` (and before any thread starts) by the loader, from `.init_array`.
    #[used]
    #[unsafe(link_section = ".init_array")]
    static HOT_TEXT: extern "C" fn() = hot_text;

    extern "C" fn hot_text() {
        let start = j2r_hot_text_start as *const () as usize;
        let end = j2r_hot_text_end as *const () as usize;
        if end <= start || end - start < ORDERED_ABOVE {
            return;
        }
        let Some((low, high)) = code_mapping(start) else { return };
        let Some((from, to)) = huge_range(start, end, low, high) else { return };
        // SAFETY: the range lies inside the executable's own code mapping; the advice changes how
        // its pages are held, not what they hold. A failure (an older kernel, a file system
        // without large folios) leaves the code on small pages, as it was.
        unsafe {
            let at = from as *mut u8;
            if madvise(at, to - from, MADV_COLLAPSE) != 0 {
                madvise(at, to - from, MADV_HUGEPAGE);
            }
        }
    }

    /// The bounds of the executable mapping that holds `address`, from `/proc/self/maps`.
    fn code_mapping(address: usize) -> Option<(usize, usize)> {
        let maps = std::fs::read_to_string("/proc/self/maps").ok()?;
        maps.lines().find_map(|line| {
            let (range, rest) = line.split_once(' ')?;
            let (low, high) = range.split_once('-')?;
            let (low, high) = (usize::from_str_radix(low, 16).ok()?, usize::from_str_radix(high, 16).ok()?);
            (rest.starts_with("r-x") && (low..high).contains(&address)).then_some((low, high))
        })
    }
}

/// The whole 2 MB blocks inside the mapping `low..high` that cover `start..end`: the blocks that
/// hold the range, less a first or last block that reaches past the mapping.
fn huge_range(start: usize, end: usize, low: usize, high: usize) -> Option<(usize, usize)> {
    let mut from = start & !(HUGE_PAGE - 1);
    if from < low {
        from += HUGE_PAGE;
    }
    let to = end.next_multiple_of(HUGE_PAGE).min(high & !(HUGE_PAGE - 1));
    (to > from).then_some((from, to))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_huge_blocks_stay_inside_the_mapping() {
        const M: usize = HUGE_PAGE;
        // Hot code from three quarters into block 0 to half into block 3, in a mapping from an
        // eighth into block 0 to half into block 4: block 0 starts before the mapping, so the
        // blocks are 1 to 3.
        assert_eq!(huge_range(3 * M / 4, 3 * M + M / 2, M / 8, 4 * M + M / 2), Some((M, 4 * M)));
        // A mapping that starts on a block: the hot range's blocks are whole.
        assert_eq!(huge_range(M + 64, 2 * M + 8, M, 10 * M), Some((M, 3 * M)));
        // Hot code inside one block that the mapping does not hold whole: nothing.
        assert_eq!(huge_range(M + 64, M + 4096, M + 32, 2 * M - 4096), None);
    }
}
