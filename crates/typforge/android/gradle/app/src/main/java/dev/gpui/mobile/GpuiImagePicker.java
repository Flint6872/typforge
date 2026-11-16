package dev.gpui.mobile;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.os.Environment;
import android.provider.MediaStore;

import java.io.File;
import java.util.ArrayList;
import java.util.concurrent.CountDownLatch;

/**
 * Image/video picker helper for gallery selection and camera capture.
 *
 * <p>All public methods are static and called from Rust via JNI.
 * They block the calling thread until the user completes or cancels the picker.</p>
 */
public final class GpuiImagePicker {

    /** Source constants matching Rust's ImageSource enum. */
    private static final int SOURCE_GALLERY = 0;
    private static final int SOURCE_CAMERA = 1;

    /** Camera facing constants matching Rust's CameraDevice enum. */
    private static final int CAMERA_REAR = 0;
    private static final int CAMERA_FRONT = 1;

    /**
     * Pick a single image from gallery or camera.
     *
     * @param activity    The current Activity.
     * @param source      0 = gallery, 1 = camera.
     * @param cameraFacing 0 = rear, 1 = front.
     * @return The image URI/path as a string, or null if cancelled.
     */
    public static String pickImage(final Activity activity, int source, int cameraFacing) {
        Intent intent;
        if (source == SOURCE_CAMERA) {
            intent = new Intent(MediaStore.ACTION_IMAGE_CAPTURE);
            if (cameraFacing == CAMERA_FRONT) {
                intent.putExtra("android.intent.extras.CAMERA_FACING", 1);
                intent.putExtra("android.intent.extras.LENS_FACING_FRONT", 1);
                intent.putExtra("android.intent.extra.USE_FRONT_CAMERA", true);
            }
        } else {
            intent = new Intent(Intent.ACTION_PICK, MediaStore.Images.Media.EXTERNAL_CONTENT_URI);
            intent.setType("image/*");
        }

        ArrayList<String> result = launchPicker(activity, intent);
        if (result != null && !result.isEmpty()) {
            return result.get(0);
        }
        return null;
    }

    /**
     * Pick multiple images from the gallery.
     *
     * @param activity The current Activity.
     * @return Array of image URIs, or null if cancelled.
     */
    public static String[] pickMultiImage(final Activity activity) {
        Intent intent = new Intent(Intent.ACTION_PICK, MediaStore.Images.Media.EXTERNAL_CONTENT_URI);
        intent.setType("image/*");
        intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);

        ArrayList<String> result = launchPicker(activity, intent);
        if (result != null) {
            return result.toArray(new String[0]);
        }
        return null;
    }

    /**
     * Pick a video from gallery or camera.
     *
     * @param activity    The current Activity.
     * @param source      0 = gallery, 1 = camera.
     * @param cameraFacing 0 = rear, 1 = front.
     * @return The video URI/path as a string, or null if cancelled.
     */
    public static String pickVideo(final Activity activity, int source, int cameraFacing) {
        Intent intent;
        if (source == SOURCE_CAMERA) {
            intent = new Intent(MediaStore.ACTION_VIDEO_CAPTURE);
            if (cameraFacing == CAMERA_FRONT) {
                intent.putExtra("android.intent.extras.CAMERA_FACING", 1);
                intent.putExtra("android.intent.extras.LENS_FACING_FRONT", 1);
                intent.putExtra("android.intent.extra.USE_FRONT_CAMERA", true);
            }
        } else {
            intent = new Intent(Intent.ACTION_PICK, MediaStore.Video.Media.EXTERNAL_CONTENT_URI);
            intent.setType("video/*");
        }

        ArrayList<String> result = launchPicker(activity, intent);
        if (result != null && !result.isEmpty()) {
            return result.get(0);
        }
        return null;
    }

    // ── Internal ─────────────────────────────────────────────────────────

    private static ArrayList<String> launchPicker(Activity activity, Intent intent) {
        CountDownLatch latch = new CountDownLatch(1);
        GpuiPickerActivity.sLatch = latch;
        GpuiPickerActivity.sResult.set(null);
        GpuiPickerActivity.sPendingIntent = intent;

        Intent proxy = new Intent(activity, GpuiPickerActivity.class);
        proxy.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        activity.startActivity(proxy);

        try {
            latch.await();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return null;
        }

        return GpuiPickerActivity.sResult.get();
    }

    private GpuiImagePicker() {}
}
