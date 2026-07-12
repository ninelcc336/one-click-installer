import "./styles.css";
import { renderReadyScreen } from "./app";

const root = document.querySelector<HTMLElement>("#app");

if (!root) {
  throw new Error("找不到应用根节点");
}

renderReadyScreen(root);
