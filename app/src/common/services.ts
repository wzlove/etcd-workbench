import {invoke} from "@tauri-apps/api";
import {Connection, ConnectionInfo, SessionData} from "~/common/transport/connection.ts";

export function _connectTest(connection: Connection): Promise<undefined> {
    return invoke('connect_test', {connection})
}

export function _connect(connection: Connection): Promise<SessionData> {
    return invoke('connect', {connection: connection})
}

export function _getConnectionList(): Promise<ConnectionInfo[]> {
    return invoke('get_connection_list')
}

export function _saveConnection(connection: ConnectionInfo) :Promise<undefined> {
    return invoke("save_connection", {connection: connection})
}