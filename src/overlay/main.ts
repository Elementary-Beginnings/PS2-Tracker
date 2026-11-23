import { mount } from "svelte";
import Overlay from "./Overlay.svelte";

const target = document.getElementById("app");
if (!target) throw new Error("#app missing");
export default mount(Overlay, { target });
