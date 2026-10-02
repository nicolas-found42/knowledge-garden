import { desktopApi } from "./api";
import { mountReader } from "./reader";
import "./style.css";

const root = document.querySelector<HTMLElement>("#app")!;
void mountReader(root, desktopApi).catch((error: unknown) => {
  const message = document.createElement("p");
  message.setAttribute("role", "alert");
  message.textContent = `The collection could not be opened: ${String(error)}`;
  root.replaceChildren(message);
});
