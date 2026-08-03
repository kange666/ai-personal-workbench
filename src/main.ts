import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import router from "./router";
import { useWorkbenchStore } from "./stores/workbench";
import "./styles/main.css";

const app = createApp(App);
const pinia = createPinia();
app.use(pinia).use(router);
void useWorkbenchStore(pinia).hydrate();
app.mount("#app");
