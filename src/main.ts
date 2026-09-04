import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import router from "./router";
import { applyFontSize, loadFontSize } from "./utils/fontSize";
import "./styles/main.css";
import "./styles/features.css";
import "./styles/internal-video.css";
import "./styles/layout-refinements.css";
import "./styles/typography.css";

// 挂载前恢复偏好，避免启动时先显示默认字号再跳变。
applyFontSize(loadFontSize());
const app = createApp(App);
const pinia = createPinia();
app.use(pinia).use(router);
app.mount("#app");
