//! Exercise the JSON surface shared by Python, Go, and Node.js.
use super::*;
use std::slice;

#[test]
fn two_circle_gradients_cross_the_c_abi_and_invalid_edits_are_atomic() {
    let mut handle = ptr::null_mut();
    assert_eq!(unsafe { ofd_new(&mut handle) }, 0);
    let commands = CString::new("M 0 0 L 30 0 L 30 30 L 0 30 C").unwrap();
    assert_eq!(
        unsafe { ofd_add_path_page(handle, commands.as_ptr(), 0., 0., 30., 30., 30., 30., 1, 0) },
        0
    );
    let id = CString::new(
        unsafe { document_ref(handle).unwrap() }
            .page_paths(0)
            .unwrap()[0]
            .id
            .clone()
            .unwrap(),
    )
    .unwrap();
    for (start, end) in [(8., 2.), (5., 5.), (8., 0.), (0., 0.)] {
        let input = serde_json::json!({"kind":"radial", "start_point":[5,15], "end_point":[20,15],
            "start_radius":start, "end_radius":end, "extend":3,
            "segments":[{"position":0,"color":[255,0,0]},{"position":1,"color":[0,0,255]}]});
        let json = CString::new(input.to_string()).unwrap();
        assert_eq!(
            unsafe {
                ofd_set_page_object_gradient_fill_json(handle, 0, id.as_ptr(), json.as_ptr())
            },
            0
        );
        assert_eq!(
            unsafe {
                ofd_set_page_object_gradient_stroke_json(handle, 0, id.as_ptr(), json.as_ptr())
            },
            0
        );
        let mut png = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_page_png(handle, 0, 25.4, &mut png) }, 0);
        assert!(unsafe { slice::from_raw_parts(png.data, png.len) }.starts_with(b"\x89PNG"));
        unsafe { ofd_buffer_free(png) };
        let mut pdf = OfdBuffer {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(unsafe { ofd_to_pdf(handle, &mut pdf) }, 0);
        assert!(unsafe { slice::from_raw_parts(pdf.data, pdf.len) }.starts_with(b"%PDF-"));
        unsafe { ofd_buffer_free(pdf) };
        let before = unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap();
        let mut invalid = input.clone();
        invalid["end_radius"] = serde_json::json!(-1);
        let json = CString::new(invalid.to_string()).unwrap();
        assert_ne!(
            unsafe {
                ofd_set_page_object_gradient_fill_json(handle, 0, id.as_ptr(), json.as_ptr())
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
        let mut coincident = input;
        coincident["start_point"] = coincident["end_point"].clone();
        coincident["start_radius"] = serde_json::json!(5);
        coincident["end_radius"] = serde_json::json!(5);
        let json = CString::new(coincident.to_string()).unwrap();
        assert_ne!(
            unsafe {
                ofd_set_page_object_gradient_fill_json(handle, 0, id.as_ptr(), json.as_ptr())
            },
            0
        );
        assert_eq!(
            unsafe { document_ref(handle).unwrap() }.to_bytes().unwrap(),
            before
        );
    }
    unsafe { ofd_close(handle) };
}
