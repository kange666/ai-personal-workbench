export const workbenchNotificationPreferenceChangedEvent = "workbench-notification-preference-changed";

const storageKey = "workbench-notifications-enabled";

export function loadWorkbenchNotificationsEnabled(): boolean {
  return window.localStorage.getItem(storageKey) !== "false";
}

export function saveWorkbenchNotificationsEnabled(enabled: boolean): void {
  window.localStorage.setItem(storageKey, String(enabled));
  window.dispatchEvent(new CustomEvent(workbenchNotificationPreferenceChangedEvent, { detail: enabled }));
}
