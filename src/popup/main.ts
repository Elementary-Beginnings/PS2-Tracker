import { mount } from "svelte";
import Popup from "./Popup.svelte";

const target = document.getElementById("app");
if (!target) throw new Error("#app missing");
export default mount(Popup, { target });
