package dev.jelon.netshare.stop;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.os.Bundle;
import android.widget.Toast;

/** 点击图标即向 gnirehtet 发送 STOP，解除电脑对手机的网络共享。 */
public class StopActivity extends Activity {
    /** gnirehtet 的包名。 */
    private static final String PKG = "com.genymobile.gnirehtet";
    /** gnirehtet 接收 START/STOP 的 Activity。 */
    private static final String TARGET = "com.genymobile.gnirehtet.GnirehtetActivity";
    /** 停止共享的 intent action。 */
    private static final String ACTION_STOP = "com.genymobile.gnirehtet.STOP";

    /** 发 STOP、提示结果、关闭自身。 */
    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        Toast.makeText(this, stop(), Toast.LENGTH_LONG).show();
        finish();
    }

    /** 发送 STOP intent，返回给用户看的结果文案。 */
    private String stop() {
        try {
            startActivity(new Intent(ACTION_STOP).setClassName(PKG, TARGET));
            return "已解除网络共享";
        } catch (SecurityException e) {
            return "未授权：请先用 adb 授予 WRITE_SECURE_SETTINGS";
        } catch (ActivityNotFoundException e) {
            return "未安装 gnirehtet";
        }
    }
}
