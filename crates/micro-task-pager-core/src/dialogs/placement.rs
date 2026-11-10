//! Placement for the Settings window.
//!
//! This mirrors the C# `AppSettingsWindow.FindPlacement` algorithm: prefer the
//! right of the pager, then the left, below, above, and finally center over the
//! pager. A candidate is accepted only when the whole dialog fits inside one
//! monitor's work area.

pub const PLACEMENT_GAP: i32 = 8;

/// A screen-space rectangle in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(self) -> i32 {
        self.right - self.left
    }

    pub fn height(self) -> i32 {
        self.bottom - self.top
    }
}

impl From<windows::Win32::Foundation::RECT> for Rect {
    fn from(rect: windows::Win32::Foundation::RECT) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        }
    }
}

/// Returns the top-left corner for a dialog of `size` relative to `owner`.
pub fn find_placement(owner: Rect, size: (i32, i32), work_areas: &[Rect]) -> (i32, i32) {
    try_right(owner, size, work_areas)
        .or_else(|| try_left(owner, size, work_areas))
        .or_else(|| try_below(owner, size, work_areas))
        .or_else(|| try_above(owner, size, work_areas))
        .unwrap_or_else(|| center_over(owner, size))
}

fn try_right(owner: Rect, size: (i32, i32), work_areas: &[Rect]) -> Option<(i32, i32)> {
    let (width, height) = size;
    let x = owner.right + PLACEMENT_GAP;
    work_areas.iter().find_map(|area| {
        (x >= area.left && x + width <= area.right && height <= area.height()).then(|| {
            let y = owner.top.clamp(area.top, area.bottom - height);
            (x, y)
        })
    })
}

fn try_left(owner: Rect, size: (i32, i32), work_areas: &[Rect]) -> Option<(i32, i32)> {
    let (width, height) = size;
    let x = owner.left - PLACEMENT_GAP - width;
    work_areas.iter().find_map(|area| {
        (x >= area.left && x + width <= area.right && height <= area.height()).then(|| {
            let y = owner.top.clamp(area.top, area.bottom - height);
            (x, y)
        })
    })
}

fn try_below(owner: Rect, size: (i32, i32), work_areas: &[Rect]) -> Option<(i32, i32)> {
    let (width, height) = size;
    let y = owner.bottom + PLACEMENT_GAP;
    work_areas.iter().find_map(|area| {
        (y >= area.top && y + height <= area.bottom && width <= area.width()).then(|| {
            let x = owner.left.clamp(area.left, area.right - width);
            (x, y)
        })
    })
}

fn try_above(owner: Rect, size: (i32, i32), work_areas: &[Rect]) -> Option<(i32, i32)> {
    let (width, height) = size;
    let y = owner.top - PLACEMENT_GAP - height;
    work_areas.iter().find_map(|area| {
        (y >= area.top && y + height <= area.bottom && width <= area.width()).then(|| {
            let x = owner.left.clamp(area.left, area.right - width);
            (x, y)
        })
    })
}

fn center_over(owner: Rect, size: (i32, i32)) -> (i32, i32) {
    (
        owner.left + (owner.width() - size.0) / 2,
        owner.top + (owner.height() - size.1) / 2,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect {
            left,
            top,
            right,
            bottom,
        }
    }

    fn owner() -> Rect {
        area(1000, 900, 1200, 950)
    }

    #[test]
    fn prefers_the_right_side() {
        let placement = find_placement(owner(), (300, 400), &[area(0, 0, 1920, 1080)]);
        assert_eq!(placement, (1208, 680));
    }

    #[test]
    fn falls_back_to_the_left_side() {
        let placement = find_placement(owner(), (800, 400), &[area(0, 0, 1920, 1080)]);
        assert_eq!(placement, (192, 680));
    }

    #[test]
    fn falls_back_below_and_above() {
        let below = find_placement(
            area(0, 300, 1920, 350),
            (500, 600),
            &[area(0, 0, 1920, 1080)],
        );
        assert_eq!(below, (0, 358));

        let above = find_placement(
            area(0, 900, 1920, 950),
            (500, 600),
            &[area(0, 0, 1920, 1080)],
        );
        assert_eq!(above, (0, 292));
    }

    #[test]
    fn clamps_to_the_work_area_edges() {
        let placement = find_placement(owner(), (300, 400), &[area(0, 0, 1920, 1000)]);
        assert_eq!(placement, (1208, 600));
    }

    #[test]
    fn centers_over_the_owner_when_nothing_fits() {
        let placement = find_placement(
            area(100, 100, 200, 150),
            (400, 400),
            &[area(0, 0, 300, 300)],
        );
        assert_eq!(placement, (-50, -75));
    }

    #[test]
    fn uses_the_monitor_that_contains_the_owner() {
        let work_areas = [area(0, 0, 1920, 1080), area(1920, 0, 3440, 1440)];
        let owner = area(2200, 900, 2400, 950);
        let placement = find_placement(owner, (500, 500), &work_areas);
        assert_eq!(placement, (2408, 900));
    }
}
