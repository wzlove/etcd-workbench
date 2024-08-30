package org.beifengtz.etcd.server.config;

import lombok.Getter;
import lombok.Setter;

import java.io.File;
import java.util.HashMap;
import java.util.Map;

/**
 * description: TODO
 * date: 15:41 2023/5/23
 *
 * @author beifengtz
 */
@Getter
@Setter
public class Configuration {

    public static final Configuration INSTANCE = new Configuration();
    public static final String DEFAULT_SYSTEM_USER = "system";

    private int port = 8080;
    private int etcdExecuteTimeoutMillis = 3000;
    private String dataDir = "data";
    private String configEncryptKey = "etcdWorkbench@*?";
    private boolean enableAuth;
    private final Map<String, String> users = new HashMap<>();
    private boolean enableHeartbeat = true;

    private Configuration() {
    }

    public File getUserDir(String user) {
        return new File(dataDir, user);
    }

    public File getUserConfigDir(String user) {
        return new File(dataDir + "/" + user + "/config");
    }

    public File getUserTokenFile(String user) {
        return new File(dataDir + "/" + user + "/token");
    }

    public void addUser(String user, String password) {
        String previous = users.put(user, password);
        if (previous != null) {
            System.err.println("Warning: exist multi user in [auth] configuration: " + user);
        }
    }
}
