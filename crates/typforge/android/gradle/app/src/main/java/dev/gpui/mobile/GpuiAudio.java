package dev.gpui.mobile;

import android.app.Activity;
import android.media.MediaPlayer;
import android.media.PlaybackParams;
import android.os.Build;
import android.util.SparseArray;

import java.io.IOException;

/**
 * Audio playback helper for the GPUI audio package.
 *
 * <p>Uses {@link MediaPlayer} for audio playback. All public methods are static
 * and called from Rust via JNI.</p>
 */
public final class GpuiAudio {

    private static final String TAG = "GpuiAudio";
    private static final SparseArray<MediaPlayer> sPlayers = new SparseArray<>();
    private static int sNextId = 1;

    /**
     * Create a new audio player.
     *
     * @return Player ID, or -1 on failure.
     */
    public static int create(Activity activity) {
        try {
            int id = sNextId++;
            MediaPlayer mp = new MediaPlayer();
            synchronized (sPlayers) {
                sPlayers.put(id, mp);
            }
            return id;
        } catch (Exception e) {
            android.util.Log.e(TAG, "create failed", e);
            return -1;
        }
    }

    /**
     * Set the audio source from a URL or file path.
     *
     * @return Duration in milliseconds, or -1 if unknown/error.
     */
    public static long setUrl(Activity activity, int id, String url) {
        MediaPlayer mp;
        synchronized (sPlayers) {
            mp = sPlayers.get(id);
        }
        if (mp == null) return -1;

        try {
            mp.reset();
            mp.setDataSource(url);
            mp.prepare();
            return mp.getDuration();
        } catch (IOException e) {
            android.util.Log.e(TAG, "setUrl failed: " + url, e);
            return -1;
        } catch (Exception e) {
            android.util.Log.e(TAG, "setUrl failed: " + url, e);
            return -1;
        }
    }

    /**
     * Start or resume playback.
     */
    public static void play(int id) {
        MediaPlayer mp;
        synchronized (sPlayers) {
            mp = sPlayers.get(id);
        }
        if (mp == null) return;

        try {
            mp.start();
        } catch (IllegalStateException e) {
            android.util.Log.e(TAG, "play failed", e);
        }
    }

    /**
     * Pause playback.
     */
    public static void pause(int id) {
        MediaPlayer mp;
        synchronized (sPlayers) {
            mp = sPlayers.get(id);
        }
        if (mp == null) return;

        try {
            if (mp.isPlaying()) {
                mp.pause();
            }
        } catch (IllegalStateException e) {
            android.util.Log.e(TAG, "pause failed", e);
        }
    }

    /**
     * Stop playback and reset to the beginning.
     */
    public static void stop(int id) {
        MediaPlayer mp;
        synchronized (sPlayers) {
            mp = sPlayers.get(id);
        }
        if (mp == null) return;

        try {
            mp.stop();
            mp.prepare();
            mp.seekTo(0);
        } catch (Exception e) {
            android.util.Log.e(TAG, "stop failed", e);
        }
    }

    /**
     * Seek to position in milliseconds.
     */
    public static void seek(int id, long positionMs) {
        MediaPlayer mp;
        synchronized (sPlayers) {
            mp = sPlayers.get(id);
        }
        if (mp == null) return;

        try {
            mp.seekTo((int) positionMs);
        } catch (IllegalStateException e) {
            android.util.Log.e(TAG, "seek failed", e);
        }
    }

    /**
     * Set volume (0.0 to 1.0).
     */
    public static void setVolume(int id, float volume) {
        MediaPlayer mp;
        synchronized (sPlayers) {
            mp = sPlayers.get(id);
        }
        if (mp == null) return;

        try {
            float v = Math.max(0.0f, Math.min(1.0f, volume));
            mp.setVolume(v, v);
        } catch (IllegalStateException e) {
            android.util.Log.e(TAG, "setVolume failed", e);
        }
    }

    /**
     * Set playback speed (requires API 23+).
     */
    public static void setSpeed(int id, float speed) {
        MediaPlayer mp;
        synchronized (sPlayers) {
            mp = sPlayers.get(id);
        }
        if (mp == null) return;

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            try {
                PlaybackParams params = mp.getPlaybackParams();
                params.setSpeed(speed);
                mp.setPlaybackParams(params);
            } catch (Exception e) {
                android.util.Log.e(TAG, "setSpeed failed", e);
            }
        } else {
            android.util.Log.w(TAG, "setSpeed requires API 23+, current: " + Build.VERSION.SDK_INT);
        }
    }

    /**
     * Set looping mode.
     */
    public static void setLooping(int id, boolean looping) {
        MediaPlayer mp;
        synchronized (sPlayers) {
            mp = sPlayers.get(id);
        }
        if (mp == null) return;

        try {
            mp.setLooping(looping);
        } catch (IllegalStateException e) {
            android.util.Log.e(TAG, "setLooping failed", e);
        }
    }

    /**
     * Get current playback position in milliseconds.
     */
    public static long getPosition(int id) {
        MediaPlayer mp;
        synchronized (sPlayers) {
            mp = sPlayers.get(id);
        }
        if (mp == null) return -1;

        try {
            return mp.getCurrentPosition();
        } catch (IllegalStateException e) {
            return -1;
        }
    }

    /**
     * Get total duration in milliseconds.
     */
    public static long getDuration(int id) {
        MediaPlayer mp;
        synchronized (sPlayers) {
            mp = sPlayers.get(id);
        }
        if (mp == null) return -1;

        try {
            return mp.getDuration();
        } catch (IllegalStateException e) {
            return -1;
        }
    }

    /**
     * Check if currently playing.
     */
    public static boolean isPlaying(int id) {
        MediaPlayer mp;
        synchronized (sPlayers) {
            mp = sPlayers.get(id);
        }
        if (mp == null) return false;

        try {
            return mp.isPlaying();
        } catch (IllegalStateException e) {
            return false;
        }
    }

    /**
     * Release the player and free resources.
     */
    public static void dispose(int id) {
        MediaPlayer mp;
        synchronized (sPlayers) {
            mp = sPlayers.get(id);
            sPlayers.remove(id);
        }
        if (mp == null) return;

        try {
            mp.release();
        } catch (Exception e) {
            android.util.Log.e(TAG, "dispose failed", e);
        }
    }

    private GpuiAudio() {}
}
