import { generateText, type LanguageModel, type ModelMessage } from "ai";
import { z } from "zod";

function extract(text: string) {
  const t = text.replace(/```(?:json)?/g, "").trim();
  const a = t.indexOf("{"), b = t.lastIndexOf("}");
  if (a < 0 || b < a) throw new Error("Model returned no JSON");
  return JSON.parse(t.slice(a, b + 1));
}

/** Portable structured output: works on every Meta model regardless of tool/json-schema support. */
export async function generateJSON<S extends z.ZodTypeAny>(opts: { model: LanguageModel; schema: S; system: string; messages: ModelMessage[] }): Promise<{ object: z.infer<S>; usage: unknown }> {
  const schemaText = JSON.stringify(z.toJSONSchema(opts.schema));
  const system = `${opts.system}\n\nRespond with ONE JSON object only, no prose, matching this JSON Schema exactly:\n${schemaText}`;
  let messages = opts.messages;
  for (let attempt = 0; attempt < 2; attempt++) {
    const r = await generateText({ model: opts.model, system, messages, temperature: 0.3, maxOutputTokens: 6000 });
    let parsed: unknown; let issue = "";
    try { parsed = extract(r.text); const ok = opts.schema.safeParse(parsed); if (ok.success) return { object: ok.data, usage: r.usage }; issue = ok.error.message.slice(0, 1500); }
    catch (e) { issue = e instanceof Error ? e.message : String(e); }
    messages = [...opts.messages, { role: "assistant", content: r.text.slice(0, 8000) }, { role: "user", content: `That JSON was invalid: ${issue}\nReturn the corrected full JSON object only.` }];
  }
  throw new Error("Model output did not match the schema after a repair attempt");
}
