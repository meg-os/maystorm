use super::*;

#[test]
fn components() {
    let rgb = ARGB8888::from_argb(0x12345678);
    let components = rgb.components();

    assert_eq!(rgb.components().a, Alpha8::new(0x12));
    assert_eq!(rgb.components().r, 0x34);
    assert_eq!(rgb.components().g, 0x56);
    assert_eq!(rgb.components().b, 0x78);

    let rgb = components.into_true_color();

    assert_eq!(rgb.argb(), 0x12345678);
}

#[test]
fn rgb555() {
    let tc_000 = ARGB8888::from_rgb(0x000000);
    let tc_00f = ARGB8888::from_rgb(0x0000FF);
    let tc_0f0 = ARGB8888::from_rgb(0x00FF00);
    let tc_f00 = ARGB8888::from_rgb(0xFF0000);
    let tc_fff = ARGB8888::from_rgb(0xFFFFFF);
    let tc_555 = ARGB8888::from_rgb(0x555555);
    let tc_aaa = ARGB8888::from_rgb(0xAAAAAA);
    let tc_5a5 = ARGB8888::from_rgb(0x55AA55);
    let tc_a5a = ARGB8888::from_rgb(0xAA55AA);

    assert_eq!(RGB555::from(tc_000).0, 0x0000);
    assert_eq!(RGB555::from(tc_00f).0, 0x001F);
    assert_eq!(RGB555::from(tc_0f0).0, 0x03E0);
    assert_eq!(RGB555::from(tc_f00).0, 0x7C00);
    assert_eq!(RGB555::from(tc_fff).0, 0x7FFF);
    assert_eq!(RGB555::from(tc_555).0, 0x294A);
    assert_eq!(RGB555::from(tc_aaa).0, 0x56B5);
    assert_eq!(RGB555::from(tc_5a5).0, 0x2AAA);
    assert_eq!(RGB555::from(tc_a5a).0, 0x5555);

    let hc_000 = RGB555(0x0000);
    let hc_00f = RGB555(0x001F);
    let hc_0f0 = RGB555(0x03E0);
    let hc_f00 = RGB555(0x7C00);
    let hc_fff = RGB555(0x7FFF);
    let hc_555 = RGB555(0x294A);
    let hc_aaa = RGB555(0x56B5);
    let hc_5a5 = RGB555(0x2AAA);
    let hc_a5a = RGB555(0x5555);

    assert_eq!(ARGB8888::from(hc_000).rgb(), 0x000000);
    assert_eq!(ARGB8888::from(hc_00f).rgb(), 0x0000FF);
    assert_eq!(ARGB8888::from(hc_0f0).rgb(), 0x00FF00);
    assert_eq!(ARGB8888::from(hc_f00).rgb(), 0xFF0000);
    assert_eq!(ARGB8888::from(hc_fff).rgb(), 0xFFFFFF);
    assert_eq!(ARGB8888::from(hc_555).rgb(), 0x525252);
    assert_eq!(ARGB8888::from(hc_aaa).rgb(), 0xADADAD);
    assert_eq!(ARGB8888::from(hc_5a5).rgb(), 0x52AD52);
    assert_eq!(ARGB8888::from(hc_a5a).rgb(), 0xAD52AD);
}

#[test]
fn canvas() {
    let true_color = ARGB8888::from_argb(0x12345678);
    let components1 = true_color.components();
    let canvas_color = RGBA8888::from(true_color);
    let components2 = canvas_color.components();
    let true_color = ARGB8888::from(canvas_color);

    assert_eq!(canvas_color.0, 0x12785634);
    assert_eq!(true_color.argb(), 0x12345678);

    assert_eq!(components1.a, components2.a);
    assert_eq!(components1.r, components2.r);
    assert_eq!(components1.g, components2.g);
    assert_eq!(components1.b, components2.b);
}

#[test]
fn one_bit_colors() {
    fn array_test(value: u8, array: &[Monochrome]) {
        let value = Octet::new(value);
        assert_eq!(value, Octet::from_array(array));
        assert_eq!(value.into_array(), array);
    }

    fn array_test2(value: u8, array: &[Monochrome], formal: &[Monochrome]) {
        let value = Octet::new(value);
        assert_eq!(value, Octet::from_array(array));
        assert_eq!(value.into_array(), formal);
    }

    array_test2(0b00000000, &[], &[Monochrome::Zero; 8]);

    array_test2(
        0b10000000,
        &[Monochrome::One],
        &[
            Monochrome::One,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::Zero,
        ],
    );

    array_test2(
        0b10100000,
        &[Monochrome::One, Monochrome::Zero, Monochrome::One],
        &[
            Monochrome::One,
            Monochrome::Zero,
            Monochrome::One,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::Zero,
        ],
    );

    array_test2(
        0b00001001,
        &[
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::One,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::One,
            Monochrome::One,
        ],
        &[
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::One,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::One,
        ],
    );

    array_test(
        0b01010101,
        &[
            Monochrome::Zero,
            Monochrome::One,
            Monochrome::Zero,
            Monochrome::One,
            Monochrome::Zero,
            Monochrome::One,
            Monochrome::Zero,
            Monochrome::One,
        ],
    );

    array_test(
        0b10101010,
        &[
            Monochrome::One,
            Monochrome::Zero,
            Monochrome::One,
            Monochrome::Zero,
            Monochrome::One,
            Monochrome::Zero,
            Monochrome::One,
            Monochrome::Zero,
        ],
    );

    array_test(
        0b00010111,
        &[
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::One,
            Monochrome::Zero,
            Monochrome::One,
            Monochrome::One,
            Monochrome::One,
        ],
    );

    array_test(
        0b11101000,
        &[
            Monochrome::One,
            Monochrome::One,
            Monochrome::One,
            Monochrome::Zero,
            Monochrome::One,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::Zero,
        ],
    );

    array_test(
        0b11000011,
        &[
            Monochrome::One,
            Monochrome::One,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::One,
            Monochrome::One,
        ],
    );

    array_test(
        0b00111100,
        &[
            Monochrome::Zero,
            Monochrome::Zero,
            Monochrome::One,
            Monochrome::One,
            Monochrome::One,
            Monochrome::One,
            Monochrome::Zero,
            Monochrome::Zero,
        ],
    );

    array_test(
        0b11111111,
        &[
            Monochrome::One,
            Monochrome::One,
            Monochrome::One,
            Monochrome::One,
            Monochrome::One,
            Monochrome::One,
            Monochrome::One,
            Monochrome::One,
        ],
    );

    let mut canvas = Octet::default();
    assert_eq!(canvas, Octet::new(0));

    canvas.set(0, Monochrome::One);
    assert_eq!(canvas, Octet::new(0b10000000));
    assert_eq!(canvas.get(0), Monochrome::One);
    assert_eq!(canvas.get(1), Monochrome::Zero);
    assert_eq!(canvas.get(2), Monochrome::Zero);
    assert_eq!(canvas.get(3), Monochrome::Zero);
    assert_eq!(canvas.get(4), Monochrome::Zero);
    assert_eq!(canvas.get(5), Monochrome::Zero);
    assert_eq!(canvas.get(6), Monochrome::Zero);
    assert_eq!(canvas.get(7), Monochrome::Zero);

    canvas.set(1, Monochrome::One);
    assert_eq!(canvas, Octet::new(0b11000000));
    assert_eq!(canvas.get(0), Monochrome::One);
    assert_eq!(canvas.get(1), Monochrome::One);

    canvas.set(2, Monochrome::One);
    assert_eq!(canvas, Octet::new(0b11100000));

    canvas.set(5, Monochrome::One);
    assert_eq!(canvas, Octet::new(0b11100100));
    assert_eq!(canvas.get(5), Monochrome::One);

    canvas.set(7, Monochrome::One);
    assert_eq!(canvas, Octet::new(0b11100101));
    assert_eq!(canvas.get(7), Monochrome::One);

    canvas.set(1, Monochrome::Zero);
    assert_eq!(canvas, Octet::new(0b10100101));
    assert_eq!(canvas.get(0), Monochrome::One);
    assert_eq!(canvas.get(1), Monochrome::Zero);
    assert_eq!(canvas.get(2), Monochrome::One);
    assert_eq!(canvas.get(3), Monochrome::Zero);
    assert_eq!(canvas.get(4), Monochrome::Zero);
    assert_eq!(canvas.get(5), Monochrome::One);
    assert_eq!(canvas.get(6), Monochrome::Zero);
    assert_eq!(canvas.get(7), Monochrome::One);
}
