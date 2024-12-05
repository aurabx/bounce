import TextInput from "@/app/components/Fields/TextInput";
import SelectInput from "@/app/components/Fields/SelectInput";

const fields = [{
    config: {
        label: 'Region',
        key: 'region',
        help: 'Select the region you login to when connecting to Aurabox',
        options: {
            au: "Australia",
            uk: "United Kingdom",
            sg: "Singapore",
        }
    },
    component: SelectInput
},{
    config: {
        label: 'Api Key',
        key: 'api_key',
        help: 'You can generate an api in the account section of your',
    },
    component: TextInput
}];



export default fields;