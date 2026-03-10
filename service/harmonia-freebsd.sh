#!/bin/sh

# PROVIDE: harmonia
# REQUIRE: NETWORKING
# KEYWORD: shutdown

. /etc/rc.subr

name="harmonia"
rcvar="harmonia_enable"

load_rc_config $name

: ${harmonia_enable:="NO"}
: ${harmonia_user:="__USER__"}
: ${harmonia_bin:="__HARMONIA_BIN__"}
: ${harmonia_log:="__HOME__/.harmoniis/harmonia/harmonia.log"}

command="${harmonia_bin}"
command_args="start --foreground >> ${harmonia_log} 2>&1 &"

pidfile="__HOME__/.harmoniis/harmonia/harmonia.pid"

harmonia_env="HARMONIA_STATE_ROOT=__HOME__/.harmoniis/harmonia"

start_cmd="${name}_start"
stop_cmd="${name}_stop"
status_cmd="${name}_status"

harmonia_start() {
    echo "Starting ${name}."
    su -l ${harmonia_user} -c "env ${harmonia_env} ${command} ${command_args}"
}

harmonia_stop() {
    echo "Stopping ${name}."
    su -l ${harmonia_user} -c "${command} stop"
}

harmonia_status() {
    su -l ${harmonia_user} -c "${command} status"
}

run_rc_command "$1"
