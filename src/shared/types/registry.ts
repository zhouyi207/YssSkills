import { z } from "zod";

const nonNegativeIntegerSchema = z.number().int().nonnegative();

export const leaderboardDtoSchema = z.enum(["allTime", "trending", "hot"]);

export type LeaderboardDto = z.infer<typeof leaderboardDtoSchema>;

export const registryResultModeDtoSchema = z.enum(["leaderboard", "search"]);

export type RegistryResultModeDto = z.infer<typeof registryResultModeDtoSchema>;

export const registrySearchRequestDtoSchema = z
  .object({
    query: z.string(),
    limit: nonNegativeIntegerSchema,
  })
  .strict();

export type RegistrySearchRequestDto = z.infer<typeof registrySearchRequestDtoSchema>;

export const registryLeaderboardRequestDtoSchema = z
  .object({
    leaderboard: leaderboardDtoSchema,
  })
  .strict();

export type RegistryLeaderboardRequestDto = z.infer<typeof registryLeaderboardRequestDtoSchema>;

export const registrySkillIdDtoSchema = z
  .object({
    source: z.string(),
    skillId: z.string(),
  })
  .strict();

export type RegistrySkillIdDto = z.infer<typeof registrySkillIdDtoSchema>;

export const registrySkillSummaryDtoSchema = z
  .object({
    id: registrySkillIdDtoSchema,
    name: z.string(),
    installs: nonNegativeIntegerSchema,
    sourceKind: z.string().nullable(),
    official: z.boolean().nullable(),
    detailsUrl: z.string().nullable(),
    rank: nonNegativeIntegerSchema.nullable(),
  })
  .strict();

export type RegistrySkillSummaryDto = z.infer<typeof registrySkillSummaryDtoSchema>;

export const registryResultDtoSchema = z.discriminatedUnion("mode", [
  z
    .object({
      mode: z.literal("leaderboard"),
      leaderboard: leaderboardDtoSchema,
      query: z.null(),
      skills: z.array(registrySkillSummaryDtoSchema),
    })
    .strict(),
  z
    .object({
      mode: z.literal("search"),
      leaderboard: z.null(),
      query: z.string(),
      skills: z.array(registrySkillSummaryDtoSchema),
    })
    .strict(),
]);

export type RegistryResultDto = z.infer<typeof registryResultDtoSchema>;
