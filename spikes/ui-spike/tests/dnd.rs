use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Bounds, Entity, Pixels, Point, TestAppContext, WindowBounds, WindowHandle,
    WindowOptions, base::Root, point, px, size,
};
use ui_spike::dnd::DndSpike;

fn open(cx: &mut TestAppContext) -> (WindowHandle<Root>, Entity<DndSpike>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        let (window, view) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1700.), px(800.)),
                })),
                ..Default::default()
            },
            cx,
            |_, cx| cx.new(|_| DndSpike::new(200)),
        )
        .unwrap();
        (window.downcast::<Root>().unwrap(), view)
    })
}

fn ids(cx: &mut TestAppContext, view: &Entity<DndSpike>, c: usize) -> Vec<usize> {
    cx.update(|cx| view.read(cx).columns[c].iter().map(|k| k.id).collect())
}

/// Drag from the middle of one card to a point `dy` from the top of another.
fn drag(
    cx: &mut TestAppContext,
    handle: WindowHandle<Root>,
    view: &Entity<DndSpike>,
    from: (usize, usize),
    to: (usize, usize),
    dy: Pixels,
) {
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let bounds = |(c, i): (usize, usize)| {
            view.read(cx).lists[c]
                .bounds_for_item(i)
                .expect("card is rendered")
        };
        let (a, b) = (bounds(from), bounds(to));
        window.drag(a.center(), point(b.center().x, b.top() + dy), cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn drag_between_two_cards_in_another_column(cx: &mut TestAppContext) {
    let (handle, view) = open(cx);
    // Card 0 of Backlog, dropped on the top edge of Review's third card.
    drag(cx, handle, &view, (0, 0), (3, 2), px(4.));
    assert_eq!(ids(cx, &view, 0).len(), 199);
    let review = ids(cx, &view, 3);
    assert_eq!(review.len(), 201);
    assert_eq!(
        &review[1..4],
        &[601, 0, 602],
        "lands between the 2nd and 3rd"
    );
    cx.update(|cx| assert_eq!(view.read(cx).slot, None, "slot cleared after the drop"));
}

#[gpui_kit::test]
fn reorder_within_a_column(cx: &mut TestAppContext) {
    let (handle, view) = open(cx);
    // Card 0 dropped on the top edge of card 3: it ends up third (after 1 and 2).
    drag(cx, handle, &view, (0, 0), (0, 3), px(4.));
    assert_eq!(&ids(cx, &view, 0)[..4], &[1, 2, 0, 3]);
}

#[gpui_kit::test]
fn only_the_visible_cards_are_laid_out(cx: &mut TestAppContext) {
    let (handle, view) = open(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let list = &view.read(cx).lists[0];
        assert!(list.bounds_for_item(0).is_some());
        assert!(
            list.bounds_for_item(199).is_none(),
            "the column is virtualised"
        );
    })
    .unwrap();
}
