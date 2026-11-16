package dev.gpui.mobile;

import android.app.Activity;
import android.media.MediaRecorder;
import android.os.Build;
import android.util.Log;

import java.io.File;
import java.io.IOException;
import java.util.UUID;

public class GpuiMicrophone {
    private static final String TAG = "GpuiMicrophone";
    private static MediaRecorder sRecorder;
    private static String sOutputPath;
    private static long sStartTime;
    private static boolean sIsRecording;
    private static boolean sIsPaused;

    public static boolean isAvailable(Activity activity) {
        return activity != null;
    }

    public static String startRecording(Activity activity, int format, int sampleRate, int channels, int bitRate) {
        if (sIsRecording) return null;
        if (activity == null) return null;

        try {
            String ext;
            int outputFormat;
            int audioEncoder;

            switch (format) {
                case 1: // WAV - not directly supported, use 3GPP as fallback
                    ext = "wav";
                    outputFormat = MediaRecorder.OutputFormat.DEFAULT;
                    audioEncoder = MediaRecorder.AudioEncoder.DEFAULT;
                    break;
                case 2: // AMR
                    ext = "amr";
                    outputFormat = MediaRecorder.OutputFormat.AMR_NB;
                    audioEncoder = MediaRecorder.AudioEncoder.AMR_NB;
                    break;
                default: // AAC
                    ext = "m4a";
                    outputFormat = MediaRecorder.OutputFormat.MPEG_4;
                    audioEncoder = MediaRecorder.AudioEncoder.AAC;
                    break;
            }

            String fileName = "recording_" + UUID.randomUUID().toString() + "." + ext;
            File outputFile = new File(activity.getCacheDir(), fileName);
            sOutputPath = outputFile.getAbsolutePath();

            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                sRecorder = new MediaRecorder(activity);
            } else {
                sRecorder = new MediaRecorder();
            }

            sRecorder.setAudioSource(MediaRecorder.AudioSource.MIC);
            sRecorder.setOutputFormat(outputFormat);
            sRecorder.setAudioEncoder(audioEncoder);
            sRecorder.setAudioSamplingRate(sampleRate);
            sRecorder.setAudioChannels(channels);
            sRecorder.setAudioEncodingBitRate(bitRate);
            sRecorder.setOutputFile(sOutputPath);

            sRecorder.prepare();
            sRecorder.start();
            sStartTime = System.currentTimeMillis();
            sIsRecording = true;
            sIsPaused = false;

            return sOutputPath;
        } catch (Exception e) {
            Log.e(TAG, "Failed to start recording", e);
            cleanup();
            return null;
        }
    }

    public static String stopRecording() {
        if (!sIsRecording || sRecorder == null) return null;

        try {
            sRecorder.stop();
            long duration = System.currentTimeMillis() - sStartTime;
            String result = sOutputPath + "|" + duration;
            cleanup();
            return result;
        } catch (Exception e) {
            Log.e(TAG, "Failed to stop recording", e);
            cleanup();
            return null;
        }
    }

    public static boolean isRecording() {
        return sIsRecording;
    }

    public static boolean pauseRecording() {
        if (!sIsRecording || sRecorder == null || sIsPaused) return false;
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            try {
                sRecorder.pause();
                sIsPaused = true;
                return true;
            } catch (Exception e) {
                Log.e(TAG, "Failed to pause recording", e);
                return false;
            }
        }
        return false;
    }

    public static boolean resumeRecording() {
        if (!sIsRecording || sRecorder == null || !sIsPaused) return false;
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            try {
                sRecorder.resume();
                sIsPaused = false;
                return true;
            } catch (Exception e) {
                Log.e(TAG, "Failed to resume recording", e);
                return false;
            }
        }
        return false;
    }

    public static double getAmplitude() {
        if (!sIsRecording || sRecorder == null || sIsPaused) return 0.0;
        try {
            int maxAmplitude = sRecorder.getMaxAmplitude();
            if (maxAmplitude <= 0) return 0.0;
            // Normalize: max amplitude for MediaRecorder is ~32767
            return Math.min(1.0, (double) maxAmplitude / 32767.0);
        } catch (Exception e) {
            return 0.0;
        }
    }

    private static void cleanup() {
        if (sRecorder != null) {
            try {
                sRecorder.release();
            } catch (Exception e) {
                // ignore
            }
            sRecorder = null;
        }
        sIsRecording = false;
        sIsPaused = false;
    }
}
