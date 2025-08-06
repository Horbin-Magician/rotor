import { createApp } from "vue";
import App from "./App.vue";
import router from "./plugins/router";
import i18n from "./plugins/i18n";

const app = createApp(App);

app.use(router);
app.use(i18n);

// mount
app.mount("#app");
