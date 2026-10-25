import { mount } from "svelte";
import "uplot/dist/uPlot.min.css";
import "./app.css";
import App from "./App.svelte";

const target = document.getElementById("app");
if (!target) throw new Error("missing #app element");

export default mount(App, { target });
