import { useFashionPreferences } from "../fashionPreferences";
import type { FashionTaskV1 } from "../contracts";
export function FashionRouting() {
  const { tasks, setTask } = useFashionPreferences();
  return (
    <section className="settings-section">
      <div>
        <h2>Fashion task preferences</h2>
        <p>
          ChatGPT is the default for contextual attributes and online research.
          Choose a compatible local vision model after comparing the same
          reviewed crop in analysis history. Availability is checked before
          sending.
        </p>
        {(["detect", "attributes"] as FashionTaskV1[]).map((task) => (
          <fieldset key={task}>
            <legend>
              {task === "detect" ? "Item suggestions" : "Fashion attributes"}
            </legend>
            <label>
              Preferred engine
              <select
                value={tasks[task].runtime}
                onChange={(e) =>
                  setTask(task, {
                    ...tasks[task],
                    runtime: e.target.value as typeof tasks.detect.runtime,
                    port: e.target.value === "lm_studio" ? 1234 : 11434,
                    model: "",
                  })
                }
              >
                <option value="">ChatGPT subscription</option>
                <option value="ollama">Local Ollama</option>
                <option value="lm_studio">Local LM Studio</option>
              </select>
            </label>
            <label>
              Preferred model ID
              <input
                value={tasks[task].model}
                onChange={(e) =>
                  setTask(task, { ...tasks[task], model: e.target.value })
                }
                placeholder="Choose in the inspection workspace"
              />
            </label>
            {tasks[task].runtime && (
              <label>
                Loopback port
                <input
                  type="number"
                  min="1"
                  max="65535"
                  value={tasks[task].port}
                  onChange={(e) =>
                    setTask(task, {
                      ...tasks[task],
                      port: Number(e.target.value),
                    })
                  }
                />
              </label>
            )}
          </fieldset>
        ))}
        <p>
          Spatial measurements, masks and catalog embeddings use the installed
          local task packs. Changing an engine never sends an image
          automatically.
        </p>
      </div>
    </section>
  );
}
