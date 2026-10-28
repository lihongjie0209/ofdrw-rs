/* Regenerate crates/ofdrw/tests/fixtures/ycck.jpg with libjpeg-turbo. */
#include <stdio.h>
#include <jpeglib.h>

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    FILE *file = fopen(argv[1], "wb");
    if (!file) return 3;

    struct jpeg_compress_struct jpeg;
    struct jpeg_error_mgr error;
    jpeg.err = jpeg_std_error(&error);
    jpeg_create_compress(&jpeg);
    jpeg_stdio_dest(&jpeg, file);
    jpeg.image_width = 2;
    jpeg.image_height = 1;
    jpeg.input_components = 4;
    jpeg.in_color_space = JCS_CMYK;
    jpeg_set_defaults(&jpeg);
    jpeg_set_colorspace(&jpeg, JCS_YCCK);
    for (int i = 0; i < 4; i++) {
        jpeg.comp_info[i].h_samp_factor = 1;
        jpeg.comp_info[i].v_samp_factor = 1;
    }
    jpeg_set_quality(&jpeg, 100, TRUE);
    jpeg_start_compress(&jpeg, TRUE);
    /* Adobe JPEG samples are inverted: red, then black. */
    JSAMPLE samples[] = {255, 0, 0, 255, 255, 255, 255, 0};
    JSAMPROW row[] = {samples};
    while (jpeg.next_scanline < jpeg.image_height) {
        jpeg_write_scanlines(&jpeg, row, 1);
    }
    jpeg_finish_compress(&jpeg);
    jpeg_destroy_compress(&jpeg);
    fclose(file);
    return 0;
}
