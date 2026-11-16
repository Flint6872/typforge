package dev.gpui.mobile;

import android.app.Activity;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothManager;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.location.LocationManager;
import android.net.Uri;
import android.os.Build;
import android.provider.Settings;

import java.util.concurrent.CountDownLatch;
import java.util.concurrent.atomic.AtomicIntegerArray;
import java.util.concurrent.atomic.AtomicReference;

/**
 * Permission handling helper for the GPUI permission_handler package.
 *
 * <p>Uses a transparent GpuiPickerActivity to handle the permission request
 * callback, since NativeActivity doesn't support onRequestPermissionsResult.</p>
 */
public final class GpuiPermissions {

    /** Status constants matching Rust's PermissionStatus enum. */
    private static final int STATUS_GRANTED = 0;
    private static final int STATUS_DENIED = 1;
    private static final int STATUS_PERMANENTLY_DENIED = 2;
    private static final int STATUS_RESTRICTED = 3;

    private static final int REQUEST_CODE = 9002;

    // ── Pending permission request state ─────────────────────────────

    static CountDownLatch sPermLatch;
    static AtomicIntegerArray sPermResults;
    static String[] sPendingPermissions;

    /**
     * Check a single permission.
     *
     * @return 0=granted, 1=denied, 2=permanently_denied
     */
    public static int checkPermission(Activity activity, String permission) {
        if (permission == null || permission.isEmpty()) return STATUS_GRANTED;

        // Special-case: SYSTEM_ALERT_WINDOW
        if ("android.permission.SYSTEM_ALERT_WINDOW".equals(permission)) {
            if (Build.VERSION.SDK_INT >= 23) {
                return Settings.canDrawOverlays(activity) ? STATUS_GRANTED : STATUS_DENIED;
            }
            return STATUS_GRANTED;
        }

        // Special-case: POST_NOTIFICATIONS (API 33+)
        if ("android.permission.POST_NOTIFICATIONS".equals(permission) && Build.VERSION.SDK_INT < 33) {
            return STATUS_GRANTED;
        }

        int result = activity.checkSelfPermission(permission);
        return (result == PackageManager.PERMISSION_GRANTED) ? STATUS_GRANTED : STATUS_DENIED;
    }

    /**
     * Request a single permission.
     *
     * <p>Launches GpuiPermissionActivity to handle the result callback.</p>
     *
     * @return 0=granted, 1=denied, 2=permanently_denied
     */
    public static int requestPermission(Activity activity, String permission) {
        if (permission == null || permission.isEmpty()) return STATUS_GRANTED;

        // Check if already granted
        if (checkPermission(activity, permission) == STATUS_GRANTED) {
            return STATUS_GRANTED;
        }

        // Special-case: SYSTEM_ALERT_WINDOW
        if ("android.permission.SYSTEM_ALERT_WINDOW".equals(permission)) {
            if (Build.VERSION.SDK_INT >= 23) {
                Intent intent = new Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                        Uri.parse("package:" + activity.getPackageName()));
                activity.startActivity(intent);
                return Settings.canDrawOverlays(activity) ? STATUS_GRANTED : STATUS_DENIED;
            }
            return STATUS_GRANTED;
        }

        return requestViaActivity(activity, new String[]{permission})[0];
    }

    /**
     * Request multiple permissions at once.
     *
     * @param permissions Pipe-separated permission strings.
     * @return Pipe-separated status ints.
     */
    public static String requestPermissions(Activity activity, String permissions) {
        if (permissions == null || permissions.isEmpty()) return "";

        String[] perms = permissions.split("\\|");
        int[] results = requestViaActivity(activity, perms);

        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < results.length; i++) {
            if (i > 0) sb.append("|");
            sb.append(results[i]);
        }
        return sb.toString();
    }

    /**
     * Check if a platform service is enabled.
     *
     * @param serviceType 0=location, 1=bluetooth
     */
    public static boolean isServiceEnabled(Activity activity, int serviceType) {
        switch (serviceType) {
            case 0: // Location
                LocationManager lm = (LocationManager) activity.getSystemService(Context.LOCATION_SERVICE);
                if (lm == null) return false;
                return lm.isProviderEnabled(LocationManager.GPS_PROVIDER)
                        || lm.isProviderEnabled(LocationManager.NETWORK_PROVIDER);
            case 1: // Bluetooth
                BluetoothManager bm = (BluetoothManager) activity.getSystemService(Context.BLUETOOTH_SERVICE);
                if (bm == null) return false;
                BluetoothAdapter adapter = bm.getAdapter();
                return adapter != null && adapter.isEnabled();
            default:
                return false;
        }
    }

    /**
     * Open the app's settings page.
     */
    public static boolean openAppSettings(Activity activity) {
        try {
            Intent intent = new Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS);
            intent.setData(Uri.parse("package:" + activity.getPackageName()));
            intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            activity.startActivity(intent);
            return true;
        } catch (Exception e) {
            android.util.Log.e("GpuiPermissions", "openAppSettings failed", e);
            return false;
        }
    }

    /**
     * Check if rationale should be shown for a permission.
     */
    public static boolean shouldShowRationale(Activity activity, String permission) {
        if (permission == null || permission.isEmpty()) return false;
        return activity.shouldShowRequestPermissionRationale(permission);
    }

    // ── Internal: request via helper Activity ────────────────────────

    private static int[] requestViaActivity(Activity activity, String[] permissions) {
        int[] results = new int[permissions.length];

        // Check which permissions are already granted
        boolean allGranted = true;
        for (int i = 0; i < permissions.length; i++) {
            if (checkPermission(activity, permissions[i]) == STATUS_GRANTED) {
                results[i] = STATUS_GRANTED;
            } else {
                results[i] = -1; // needs request
                allGranted = false;
            }
        }

        if (allGranted) return results;

        // Filter to only ungranted permissions
        java.util.List<String> needed = new java.util.ArrayList<>();
        java.util.List<Integer> neededIndices = new java.util.ArrayList<>();
        for (int i = 0; i < permissions.length; i++) {
            if (results[i] == -1) {
                needed.add(permissions[i]);
                neededIndices.add(i);
            }
        }

        String[] neededArray = needed.toArray(new String[0]);

        // Use GpuiPermissionActivity for the request
        CountDownLatch latch = new CountDownLatch(1);
        GpuiPermissionActivity.sLatch = latch;
        GpuiPermissionActivity.sPermissions = neededArray;
        GpuiPermissionActivity.sResults = new AtomicIntegerArray(neededArray.length);
        for (int i = 0; i < neededArray.length; i++) {
            GpuiPermissionActivity.sResults.set(i, STATUS_DENIED);
        }

        Intent intent = new Intent(activity, GpuiPermissionActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        activity.startActivity(intent);

        try {
            latch.await();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }

        // Map results back
        for (int i = 0; i < neededIndices.size(); i++) {
            int idx = neededIndices.get(i);
            int grantResult = GpuiPermissionActivity.sResults.get(i);
            if (grantResult == PackageManager.PERMISSION_GRANTED) {
                results[idx] = STATUS_GRANTED;
            } else {
                // Check if permanently denied (shouldShowRequestPermissionRationale returns false after denial)
                if (!activity.shouldShowRequestPermissionRationale(neededArray[i])) {
                    results[idx] = STATUS_PERMANENTLY_DENIED;
                } else {
                    results[idx] = STATUS_DENIED;
                }
            }
        }

        return results;
    }

    private GpuiPermissions() {}
}
