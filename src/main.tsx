import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { SkillManagerApp } from "./App";
import { createTauriSkillGateway } from "./gateway";

const root = document.getElementById("root");

if (!root) throw new Error("缺少应用挂载节点");

createRoot(root).render(
  <StrictMode>
    <SkillManagerApp gateway={createTauriSkillGateway()} />
  </StrictMode>,
);
