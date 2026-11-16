package dev.gpui.mobile;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;

public class GpuiDeepLink {

    public static String getInitialLink(Activity activity) {
        if (activity == null) return null;
        Intent intent = activity.getIntent();
        if (intent == null) return null;
        String action = intent.getAction();
        Uri data = intent.getData();
        if (data != null && (Intent.ACTION_VIEW.equals(action) || Intent.ACTION_MAIN.equals(action))) {
            return data.toString();
        }
        return null;
    }
}
