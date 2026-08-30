import { z } from "zod";

const nonNegativeIntegerSchema = z.number().int().nonnegative();
const integerSchema = z.number().int();

export const pathDtoSchema = z
  .object({
    value: z.string().nullable(),
    display: z.string(),
  })
  .strict();

export type PathDto = z.infer<typeof pathDtoSchema>;

export const retryAfterDtoSchema = z.discriminatedUnion("kind", [
  z
    .object({
      kind: z.literal("delay"),
      seconds: nonNegativeIntegerSchema,
    })
    .strict(),
  z
    .object({
      kind: z.literal("at"),
      epochMillis: integerSchema,
    })
    .strict(),
]);

export type RetryAfterDto = z.infer<typeof retryAfterDtoSchema>;

export const ipcErrorSchema = z
  .object({
    code: z.string(),
    message: z.string(),
    retryable: z.boolean(),
    context: z.record(z.string(), z.string()).optional(),
    retryAfter: retryAfterDtoSchema.optional(),
  })
  .strict();

export type IpcError = z.infer<typeof ipcErrorSchema>;
