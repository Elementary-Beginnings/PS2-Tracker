import { mount } from "svelte";
import Move from "./Move.svelte";

const target = document.getElementById("app");
if (!target) throw new Error("#app missing");
export default mount(Move, { target });
