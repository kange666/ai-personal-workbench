use rusqlite::Connection;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct AccessState {
    connections: usize,
    restoring: bool,
    restart_required: bool,
}

#[derive(Default)]
pub(super) struct DatabaseAccess {
    state: Mutex<AccessState>,
    idle: Condvar,
}

impl DatabaseAccess {
    pub(super) fn lease(self: &Arc<Self>) -> Result<ConnectionLease, String> {
        let mut state = self.state.lock().map_err(|_| "数据库访问锁异常")?;
        if state.restart_required {
            return Err("数据已恢复，请重启工作台后继续操作。".into());
        }
        if state.restoring {
            return Err("正在恢复本地数据，请稍后重试。".into());
        }
        state.connections += 1;
        Ok(ConnectionLease(self.clone()))
    }

    pub(super) fn begin_restore(
        self: &Arc<Self>,
        timeout: Duration,
    ) -> Result<DatabaseRestoreGuard, String> {
        let mut state = self.state.lock().map_err(|_| "数据库访问锁异常")?;
        if state.restoring || state.restart_required {
            return Err("已有数据恢复正在进行或等待重启。".into());
        }
        state.restoring = true;
        let deadline = Instant::now() + timeout;
        while state.connections != 0 {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                state.restoring = false;
                return Err("后台任务仍在使用数据库，本次未恢复数据，请稍后重试。".into());
            }
            state = match self.idle.wait_timeout(state, remaining) {
                Ok((state, _)) => state,
                Err(error) => {
                    let (mut state, _) = error.into_inner();
                    state.restoring = false;
                    return Err("等待数据库任务结束失败，本次未恢复数据。".into());
                }
            };
        }
        Ok(DatabaseRestoreGuard {
            access: self.clone(),
        })
    }
}

pub(super) struct ConnectionLease(Arc<DatabaseAccess>);

impl Drop for ConnectionLease {
    fn drop(&mut self) {
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.connections -= 1;
        self.0.idle.notify_all();
    }
}

// 字段按声明顺序释放：先关闭 SQLite，再释放租约，恢复过程才可安全接管文件。
pub struct DatabaseConnection {
    pub(super) connection: Connection,
    pub(super) _lease: ConnectionLease,
}

impl Deref for DatabaseConnection {
    type Target = Connection;
    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl DerefMut for DatabaseConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}

pub(crate) struct DatabaseRestoreGuard {
    access: Arc<DatabaseAccess>,
}

impl DatabaseRestoreGuard {
    // 成功后保持封闭直到进程重启，避免恢复前的后台计算结果重新写入恢复后的数据。
    pub(crate) fn require_restart(&self) {
        let mut state = self
            .access
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.restart_required = true;
    }
}

impl Drop for DatabaseRestoreGuard {
    fn drop(&mut self) {
        let mut state = self
            .access
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.restoring = false;
        self.access.idle.notify_all();
    }
}
