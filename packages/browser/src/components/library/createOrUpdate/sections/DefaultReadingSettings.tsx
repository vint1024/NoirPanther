import { Label, NativeSelect, Text } from '@stump/components'
import { useLocaleContext } from '@stump/i18n'
import { useFormContext } from 'react-hook-form'

import { CreateOrUpdateLibrarySchema } from '../schema'

export default function DefaultReadingSettings() {
	const form = useFormContext<CreateOrUpdateLibrarySchema>()

	const { t } = useLocaleContext()

	return (
		<>
			<div className="gap-2 flex items-center">
				<div className="gap-2 flex flex-col">
					<Label>{t(getKey('imageScaling.label'))}</Label>
					<NativeSelect
						options={[
							{ label: 'Auto', value: 'AUTO' },
							{ label: 'Height', value: 'HEIGHT' },
							{ label: 'Width', value: 'WIDTH' },
							{ label: 'Original', value: 'NONE' },
						]}
						{...form.register('defaultReadingImageScaleFit')}
					/>
					<Text size="xs" variant="muted">
						{t(getKey('imageScaling.description'))}
					</Text>
				</div>

				<div className="gap-2 flex flex-col">
					<Label>{t(getKey('readingDirection.label'))}</Label>
					<NativeSelect
						options={[
							{
								label: t(
									'settingsScene.app/reader.sections.universal.sections.readingDirection.ltr',
								),
								value: 'LTR',
							},
							{
								label: t(
									'settingsScene.app/reader.sections.universal.sections.readingDirection.rtl',
								),
								value: 'RTL',
							},
						]}
						{...form.register('defaultReadingDir')}
					/>
					<Text size="xs" variant="muted">
						{t(getKey('readingDirection.description'))}
					</Text>
				</div>
			</div>

			<div className="gap-2 md:w-2/3 flex flex-col">
				<Label>{t(getKey('readingMode.label'))}</Label>
				<NativeSelect
					options={[
						{
							label: t('imageReader.settings.readingMode.options.verticalScroll'),
							value: 'CONTINUOUS_VERTICAL',
						},
						{
							label: t('imageReader.settings.readingMode.options.horizontalScroll'),
							value: 'CONTINUOUS_HORIZONTAL',
						},
						{ label: 'Paged', value: 'PAGED' },
					]}
					{...form.register('defaultReadingMode')}
				/>
				<Text size="xs" variant="muted">
					{t(getKey('readingMode.description'))}
				</Text>
			</div>
		</>
	)
}

const LOCALE_KEY = 'createOrUpdateLibraryForm.fields.readingSettings'
const getKey = (key: string) => `${LOCALE_KEY}.${key}`
