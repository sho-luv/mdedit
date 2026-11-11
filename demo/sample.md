# Trip Planner

A small **command-line tool** that turns a list of cities into a day-by-day
itinerary. Docs live at https://example.com/trip-planner.

## Quick start

1. Install it with `cargo install trip-planner`
2. Add your cities to `trip.toml`
3. Run `trip plan --days 5`

```rust
fn main() {
    // build the itinerary and print it
    let trip = Trip::load("trip.toml");
    println!("{} stops planned", trip.stops().len());
}
```

## Status

- [x] Parse the city list
- [x] Estimate travel times
- [ ] Export to calendar

| City | Nights | Notes |
|------|:------:|-------|
| Lisbon | 2 | Arrive by train |
| Porto | 3 | Book the river tour |

> [!NOTE]
> Travel times are estimates. Check them before you book.
