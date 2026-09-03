import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import router from "./router";
import "./styles/main.css";
import "./styles/features.css";
import "./styles/internal-video.css";

const app = createApp(App);
const pinia = createPinia();
app.use(pinia).use(router);
app.mount("#app");
